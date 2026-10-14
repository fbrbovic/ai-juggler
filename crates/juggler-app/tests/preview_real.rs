//! AJ-E415 real-capture evidence: drives the REAL `ScreenshotSource`
//! (`ScreenCaptureKit`) once. On a machine without the Screen Recording
//! grant this records the honest DENIED path end to end (no prompt:
//! content listing errors cleanly). With the grant (maintainer
//! machine), it captures one real window, prints frame sizes/timings
//! and the CPU delta — the live record. Ignored in CI (no GUI/no
//! grant): run locally with
//! `cargo test -p juggler-app --test preview_real -- --ignored
//! --nocapture`.

#![cfg(target_os = "macos")]
// Test crates are pragmatic by policy (QUALITY.md §2).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic
)]

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use juggler_app::preview::{
    CaptureError, CaptureSource, PreviewDriver, PreviewEvent, ScreenshotSource,
};

fn cpu_percent() -> Option<f32> {
    let out = std::process::Command::new("ps")
        .args(["-o", "%cpu=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f32>()
        .ok()
}

#[test]
#[ignore = "drives real ScreenCaptureKit; local evidence run only"]
fn real_capture_record() {
    let source = ScreenshotSource;
    let identity = juggler_core::model::WindowIdentity {
        pid: 0, // forces the by-pid miss; the source falls back to its
        // content enumeration — with no grant this is the denied probe
        bundle_id: Some("com.apple.TextEdit".into()),
        title: Some("aij-e415-probe".into()),
        cgwindow_id: None,
    };
    let started = Instant::now();
    let attempt = source.capture(&identity);
    let elapsed = started.elapsed();
    match attempt {
        Err(CaptureError::PermissionDenied) => {
            println!(
                "[record] DENIED in {elapsed:?}: Screen Recording grant missing — \
                 the panel renders guidance + status data only (Req.3 path)"
            );
            // The driver over the real source: denied is terminal and
            // the loop ends without a frame (teardown by construction).
            let events: Arc<Mutex<Vec<PreviewEvent>>> = Arc::new(Mutex::new(Vec::new()));
            let sink = events.clone();
            let emit = move |event: PreviewEvent| sink.lock().unwrap().push(event);
            let stopped = AtomicBool::new(false);
            let driver = PreviewDriver {
                session_id: "sess-real",
                identity,
                source: &source,
                is_live: &|| true,
                emit: &emit,
                stopped: &stopped,
            };
            let loop_started = Instant::now();
            driver.run();
            assert!(
                loop_started.elapsed() < Duration::from_secs(1),
                "denied loop must end immediately"
            );
            let events = events.lock().unwrap().clone();
            assert!(matches!(
                events.as_slice(),
                [PreviewEvent::Phase { phase, .. }] if *phase == juggler_app::preview::PreviewPhase::Denied
            ));
            println!("[record] driver emitted exactly one Denied phase, no frames");
        }
        Err(other) => panic!("unexpected capture error: {other:?}"),
        Ok(png) => {
            // The granted-machine live record: one real frame.
            assert!(
                png.len() > 8 && png.starts_with(&[0x89, b'P', b'N', b'G']),
                "not a PNG: {} bytes",
                png.len()
            );
            println!(
                "[record] LIVE frame: {} bytes PNG in {elapsed:?}; probing 2 s of polling…",
                png.len()
            );
            let before = cpu_percent();
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut frames = 0usize;
            while Instant::now() < deadline {
                if source.capture(&identity).is_ok() {
                    frames += 1;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            let during = cpu_percent();
            let after = cpu_percent();
            println!(
                "[record] 2 s polling: {frames} frames; cpu before={before:?}% during={during:?}% after={after:?}%"
            );
        }
    }
}
