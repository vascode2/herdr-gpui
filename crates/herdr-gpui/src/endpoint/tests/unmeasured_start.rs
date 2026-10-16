use super::*;

/// A window connects before its first layout. The local daemon must not be
/// asked for a surface then: it would size every shown pane to the
/// placeholder, and each program in them would redraw again once the
/// measured size followed.
#[gpui::test]
fn the_local_daemon_is_asked_for_a_surface_only_once_the_terminal_is_measured(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    view.update(cx, |view, cx| {
        // An explicit socket that does not exist: never the user's daemon,
        // and an explicit socket is never started.
        let missing =
            std::env::temp_dir().join(format!("herdr-gpui-unmeasured-{}.sock", std::process::id()));
        view.selected_endpoint = 0;
        view.endpoints[0].connection.target = ConnectTarget::Socket(missing);

        view.terminal_measured = false;
        view.reconnect(cx);
        assert!(!view.endpoints[0].surface_requested());

        view.terminal_measured = true;
        view.reconnect(cx);
        assert!(view.endpoints[0].surface_requested());
        view.endpoints[0].stop();
    });
}
