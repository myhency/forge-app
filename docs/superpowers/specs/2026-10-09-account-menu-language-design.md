# Account menu and language (Plan 36, stage 1 of the translation)

Roadmap: PG-08 (account menu). This spec also starts the translation of the whole app into Korean and English; this stage is stage 1 of 6.

## Intent

The person wants the account menu of the desktop app at the bottom of the sidebar, with a language choice of Korean or English. The language applies to **the whole app**. That is too large for one plan, so it is built in six stages. Stage 1 (this spec) builds the translation structure and the account menu, and translates only the account menu. Every other screen stays English until its stage.

Said by the person:
- Menu items to build: Settings, Usage, Language, Get help, Learn more, Get API key.
- Get help, Learn more, Get API key and Usage are shown only; they do nothing.
- Language: Korean or English.
- Learn more has a submenu with Usage policy, Privacy policy and Keyboard shortcuts (the shortcut is shown).
- The language covers the whole app, not only this menu.
- First start: follow the system language.
- The top of the account area shows the OS user name.

Assumed (not said): the translation method below (chosen from three options), and that the menu rows are in the order of the desktop app (round 15 of the observations).

## Stages of the translation

1. This spec: structure, account menu.
2. Sidebar, session menu, search palette, filter menu.
3. Chat view, composer, permission and question panels, new-session screen.
4. Dock panels: terminal, changes, files, preview.
5. Pages: home dashboard, routines, customize.
6. Settings window and the shortcut list.

Each stage has its own spec, plan and merge. An untranslated string shows in English, so the app works after every stage.

## Translation structure

Module `i18n` in `forge-shared`.

- `Language { English, Korean }`, stored in `AppConfig` like `Theme` (`choice_enum!`, serialised value `"en"` / `"ko"`). `AppConfig.language` is `Option<Language>`: `None` means "not chosen yet", and the system language decides. A value that cannot be read becomes `None`.
- System language: Korean when the system's first preferred language starts with `ko`; English otherwise, and when it cannot be read. How to read it on macOS (the app started from the Dock has no `LANG`) and Windows is decided in the plan; a spike comes first.
- The current language is a global Dioxus signal. Changing it redraws every view that calls `tr!`, with no restart. The choice is written to the config at once.
- `tr!("Settings")` returns the Korean text when the language is Korean and the table has the sentence; otherwise the English sentence itself. The English sentence is the key.
- With values: `tr!("{n} sessions", n = count)`. The Korean text has the same placeholders. Placeholders are replaced by name.
- The Korean table is one static table in `ko.rs`, sorted by key.
- Plural forms stay the job of the calling code (Korean has none).

### Test that guards the table

A test scans the sources of every crate for `tr!("...")` and checks:
- every sentence found has a Korean text;
- every Korean entry is used somewhere (no leftovers);
- a Korean text uses exactly the placeholders of its key.

A sentence built at run time (`tr!(some_variable)`) is refused by the macro: it takes a string literal only.

## Account menu

File `forge-widgets/src/account_menu.rs`.

**Account area.** Last row of the sidebar: the OS user name (`USER`, on Windows `USERNAME`; "Forge" when neither is set) and a chevron. Clicking opens the menu above it; a click outside or Escape closes it. The menu follows `context_menu.rs`, and the submenus follow the same code.

**Rows**, top to bottom:

| Row | Action |
| --- | --- |
| user name (text only) | none |
| divider | |
| Settings `⌘,` | `Settings::open()`, close the menu |
| Usage | shown only; closes the menu |
| Language `>` | submenu: 한국어, English; a check beside the current one; choosing one sets the language and closes the menu |
| Get help | shown only; closes the menu |
| divider | |
| Learn more `>` | submenu: Usage policy, Privacy policy (both shown only), divider, Keyboard shortcuts `⌘/` (opens the existing list, Plan 32B) |
| divider | |
| Get API key | shown only; closes the menu |

"Shown only" rows look like normal rows. They are not greyed, as in the desktop app; they just close the menu. Language names are written in their own language and are never translated.

`⌘,` opens Settings if Plan 33 already binds it; if not, this stage binds it. Checked while building.

## Translated in this stage

The user-name row has no text of its own. The menu rows above, and the account area's tooltip, are translated. Nothing else.

Korean wording follows the desktop app (observations, round 15): 설정, 사용량, 언어, 도움 받기, 더 알아보기, API 키 받기, 이용 정책, 개인정보 처리방침, 단축키.

## Tests and checks

Unit tests:
- the translation function: English, Korean, a sentence missing from the table, placeholders, a placeholder missing from a value;
- the table test above;
- `AppConfig` reading and writing `language`, an unknown value, and an old file without the field;
- the system-language rule on sample values (`ko-KR`, `ko`, `en-US`, empty, garbage);
- the menu model: rows, order, which rows have a submenu, what choosing a language does.

Real window (the harness of Plans 13 and 26): open and close the menu (click, outside click, Escape), both submenus, switching to Korean and back with no restart, the choice kept after a restart, Settings and the shortcut list opening from the menu, first start with a Korean and an English system language, a narrow window (700 pt and below, Plan 26) where the sidebar is an overlay.

## Not in this stage

- Translating any screen but the account menu.
- Any action behind Usage, Get help, Usage policy, Privacy policy, Get API key.
- The rows the desktop app has and the person left out: upgrade plan, download apps, guest pass, changelog, Anthropic, Academy, terms, privacy choices, log out; the icon beside the account name.
- A Forge account or an email: the user name is the OS name.
