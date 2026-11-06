(function(){
var ARGS=__ARGS__;
var state=window.__forgeRefs;var w=state&&state.byRef.get(ARGS.ref);var el=w&&w.deref();
if(!el)return 'Error: no element '+ARGS.ref+' on this page; call read_page first.';
var v=ARGS.value;
if(el.tagName==='INPUT'&&(el.type==='checkbox'||el.type==='radio')){el.checked=(v===true||v==='true');}
else if(el.tagName==='SELECT'){var o=Array.prototype.find.call(el.options,function(o){return o.value===String(v)||o.text.trim()===String(v);});if(!o)return 'Error: '+ARGS.ref+' has no option '+v;el.value=o.value;}
else if(el.isContentEditable){el.textContent=String(v);}
else if('value' in el){var proto=el.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;var setter=Object.getOwnPropertyDescriptor(proto,'value');if(setter&&setter.set)setter.set.call(el,String(v));else el.value=String(v);}
else return 'Error: '+ARGS.ref+' is not a form field.';
el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new Event('change',{bubbles:true}));
return 'Set '+ARGS.ref+' to '+JSON.stringify(v)+'.';
})()
