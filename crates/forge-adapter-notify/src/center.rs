//! The macOS notification center (`UNUserNotificationCenter`). It labels a notification with the app's own
//! name and icon and tells Forge which one was clicked, which `osascript` cannot do.

use std::sync::{Arc, Mutex};

use block2::RcBlock;
use forge_ports::ClickHandler;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AnyThread, DefinedClass};
use objc2_foundation::{NSBundle, NSDictionary, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

/// The key under which a notification keeps the `target` it was sent with.
const TARGET_KEY: &str = "forgeTarget";

type Slot = Arc<Mutex<Option<ClickHandler>>>;

struct DelegateState {
    click: Slot,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "ForgeNotificationDelegate"]
    #[ivars = DelegateState]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        // Show the banner even while Forge is the front app (the person may be looking at another session).
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(&self, _center: &UNUserNotificationCenter, _notification: &UNNotification, handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>) {
            handler.call((UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::Sound,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(&self, _center: &UNUserNotificationCenter, response: &UNNotificationResponse, handler: &block2::DynBlock<dyn Fn()>) {
            let info = response.notification().request().content().userInfo();
            let target = info
                .objectForKey(&NSString::from_str(TARGET_KEY))
                .and_then(|value| value.downcast::<NSString>().ok())
                .map(|text| text.to_string());
            if let Some(target) = target {
                if let Some(click) = self.ivars().click.lock().unwrap().as_ref() {
                    click(target);
                }
            }
            handler.call(());
        }
    }
);

impl Delegate {
    fn new(click: Slot) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateState { click });
        unsafe { msg_send![super(this), init] }
    }
}

/// Sends notifications through the notification center and reports clicks.
pub(crate) struct NotificationCenter {
    click: Slot,
    // The center holds its delegate weakly: this keeps it alive.
    _delegate: Retained<Delegate>,
}

// The delegate is only touched by the system's threads through the methods above, which only read `click`
// behind its mutex.
unsafe impl Send for NotificationCenter {}
unsafe impl Sync for NotificationCenter {}

impl NotificationCenter {
    /// Asks the system for permission and starts listening for clicks.
    pub(crate) fn start() -> Self {
        let click: Slot = Arc::new(Mutex::new(None));
        let delegate = Delegate::new(click.clone());
        let center = UNUserNotificationCenter::currentNotificationCenter();
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        let done = RcBlock::new(|_granted: objc2::runtime::Bool, _error: *mut NSError| {});
        center.requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound, &done);
        Self { click, _delegate: delegate }
    }

    pub(crate) fn set_click_handler(&self, handler: ClickHandler) {
        *self.click.lock().unwrap() = Some(handler);
    }

    pub(crate) fn send(&self, title: &str, body: &str, target: Option<&str>) -> Result<(), String> {
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        if let Some(target) = target {
            let key = NSString::from_str(TARGET_KEY);
            let value = NSString::from_str(target);
            let value: Retained<AnyObject> = Retained::into_super(Retained::into_super(value));
            let typed: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_retained_objects(&[&*key], &[value]);
            // The same object, read with the loose type `setUserInfo` names.
            let info: Retained<NSDictionary> = unsafe { Retained::cast_unchecked(typed) };
            unsafe { content.setUserInfo(&info) };
        }
        let identifier = NSString::from_str(&format!("forge-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_nanos()).unwrap_or(0)));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&identifier, &content, None);
        UNUserNotificationCenter::currentNotificationCenter().addNotificationRequest_withCompletionHandler(&request, None);
        Ok(())
    }
}

/// The identifier of the running app bundle, `None` for a bare binary.
pub(crate) fn bundle_identifier() -> Option<String> {
    NSBundle::mainBundle().bundleIdentifier().map(|identifier| identifier.to_string())
}
