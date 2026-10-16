use super::*;

/// Activation sizes the daemon's panes for this window, so it waits for
/// layout to measure the terminal, and the first size it sends is that one.
#[gpui::test]
fn activation_waits_for_the_measured_terminal_and_sends_its_size(cx: &mut gpui::TestAppContext) {
    let (fixture, cx) = cx.add_window_view(|window, cx| {
        Fixture(cx.new(|cx| crate::sidebar::layout_tests::fixture_window(window, cx)))
    });
    let view = fixture.read_with(cx, |fixture, _| fixture.0.clone());
    let (mut endpoint, mut server) = connected_endpoint("ssh:measured");
    endpoint.initial_surface = false;
    let measured = ConnectOptions {
        surface_size: ClientSurfaceSize {
            cols: 132,
            rows: 43,
        },
        cell_width_px: 9,
        cell_height_px: 18,
    };
    view.update(cx, |view, cx| {
        view.terminal_measured = false;
        view.endpoints[0].detached = true;
        view.endpoints.push(endpoint);
        view.selected_endpoint = 1;
        view.reset_selected(cx);
        view.poll_endpoints(cx);
        assert!(!view.endpoints[1].surface_requested());
        assert!(view.live.activation.is_none());

        view.options = measured;
        view.terminal_measured = true;
        project_until(view, cx, "activation", |view| {
            view.endpoints[1].surface_requested()
        });
    });
    let size = loop {
        if let ClientMessage::ClientShellResize { surface_size, .. } = server.receive() {
            break surface_size;
        }
    };
    assert_eq!(size, measured.surface_size);
}
