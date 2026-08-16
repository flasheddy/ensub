use ensub_gui::{update_hud, HudEffect, HudMessage, HudModel};

#[test]
fn hud_reads_clipboard_once_and_keeps_editable_fallback() {
    let mut model = HudModel::default();

    let effects = update_hud(&mut model, HudMessage::Opened);
    assert_eq!(effects, vec![HudEffect::ReadClipboard]);
    assert!(update_hud(&mut model, HudMessage::Opened).is_empty());

    let _ = update_hud(
        &mut model,
        HudMessage::ClipboardLoaded(Err("unavailable".to_string())),
    );
    let _ = update_hud(
        &mut model,
        HudMessage::TextChanged("editable text".to_string()),
    );
    assert_eq!(model.text, "editable text");
    assert!(!model.closed);
}

#[test]
fn hud_closes_only_after_successful_capture_or_cancel() {
    let mut model = HudModel::default();
    let _ = update_hud(
        &mut model,
        HudMessage::CaptureFinished(Err("busy".to_string())),
    );
    assert!(!model.closed);
    assert_eq!(model.error.as_deref(), Some("busy"));

    let _ = update_hud(&mut model, HudMessage::CaptureFinished(Ok(2)));
    assert!(model.closed);

    let mut cancelled = HudModel::default();
    let _ = update_hud(&mut cancelled, HudMessage::Cancel);
    assert!(cancelled.closed);
}
