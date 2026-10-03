# Plan 16 implementation: composer controls

Spec: `docs/superpowers/specs/2026-10-03-composer-controls-design.md`. TDD, one commit per task.

1. **Domain**: `PermissionMode::Auto`; wire mode; loader; new-session select.
2. **Sidecar and port**: TS commands and events, `allowDangerouslySkipPermissions`; Rust `SidecarCommand`, `SidecarEvent`, `SidecarPort::set_permission_mode` and `set_model`, adapter, fake.
3. **Application**: `apply_sidecar_event` for the three events, `LiveCommand`, handle methods, `LiveSessionView.model`, actor.
4. **Menu logic** (pure): mode rows and model rows with descriptions and keys, key to row, the bypass confirmation.
5. **Pickers**: widgets, CSS, placed in the chat window's composer.
6. **Check and docs**: harness run with a real message; backlog (CMP-03, CMP-04, CMP-05 note); as-built notes; memory.
