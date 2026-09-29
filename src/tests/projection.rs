#![allow(clippy::unwrap_used, clippy::expect_used)]

use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::projection::{letterbox, overview_columns, Projection, ProjectionError, ProjectionKind};

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rectangle<f64, Logical> {
    Rectangle::new(Point::from((x, y)), Size::from((w, h)))
}

fn size(w: f64, h: f64) -> Size<f64, Logical> {
    Size::from((w, h))
}

#[test]
fn round_trip_to_source_and_to_viewer() {
    let projection = Projection::new(
        "viewer".to_string(),
        "source".to_string(),
        rect(0., 0., 1728., 1080.),
        rect(1408., 0., 2304., 1440.),
        ProjectionKind::View,
    )
    .unwrap();

    let source_point = Point::from((100., 200.));
    let viewer_point = projection.to_viewer(source_point);
    let round_tripped = projection.to_source(viewer_point).unwrap();

    assert!((round_tripped.x - source_point.x).abs() < 1e-9);
    assert!((round_tripped.y - source_point.y).abs() < 1e-9);
}

#[test]
fn to_source_outside_region_is_none() {
    let projection = Projection::new(
        "viewer".to_string(),
        "source".to_string(),
        rect(0., 0., 1728., 1080.),
        rect(1408., 0., 2304., 1440.),
        ProjectionKind::View,
    )
    .unwrap();

    assert_eq!(projection.to_source(Point::from((0., 0.))), None);
    assert_eq!(projection.to_source(Point::from((1407., 100.))), None);
    assert_eq!(
        projection.to_source(Point::from((1408. + 2304., 100.))),
        None
    );
}

#[test]
fn scale_for_landscape_letterbox_into_5120x1440() {
    // 1728x1080 source letterboxed into a 5120x1440 viewer: region height 1440,
    // width 2304, x offset 1408.
    let projection = Projection::new(
        "viewer".to_string(),
        "source".to_string(),
        rect(0., 0., 1728., 1080.),
        rect(1408., 0., 2304., 1440.),
        ProjectionKind::View,
    )
    .unwrap();

    assert!((projection.scale() - 2304. / 1728.).abs() < 1e-9);
}

#[test]
fn scale_for_portrait_letterbox_into_1440x2560() {
    // 1280x800 source letterboxed into a 1440x2560 portrait viewer: region
    // width 1440, height 900, y offset 830.
    let projection = Projection::new(
        "viewer".to_string(),
        "source".to_string(),
        rect(0., 0., 1280., 800.),
        rect(0., 830., 1440., 900.),
        ProjectionKind::View,
    )
    .unwrap();

    assert!((projection.scale() - 1440. / 1280.).abs() < 1e-9);
}

#[test]
fn constructor_rejects_empty_source_rect() {
    let err = Projection::new(
        "viewer".to_string(),
        "source".to_string(),
        rect(0., 0., 0., 1080.),
        rect(0., 0., 100., 100.),
        ProjectionKind::View,
    )
    .unwrap_err();

    assert!(matches!(err, ProjectionError::EmptySourceRect { .. }));
}

#[test]
fn constructor_rejects_empty_region() {
    let err = Projection::new(
        "viewer".to_string(),
        "source".to_string(),
        rect(0., 0., 1728., 1080.),
        rect(0., 0., 2304., 0.),
        ProjectionKind::View,
    )
    .unwrap_err();

    assert!(matches!(err, ProjectionError::EmptyRegion { .. }));
}

#[test]
fn constructor_rejects_non_finite_rects() {
    let good = rect(0., 0., 1728., 1080.);
    for bad in [
        // A NaN size cannot be built at all: smithay's `Size` asserts on it in debug builds.
        rect(f64::NAN, 0., 1728., 1080.),
        rect(0., 0., f64::INFINITY, 1080.),
        rect(0., f64::NEG_INFINITY, 1728., 1080.),
    ] {
        let as_source = Projection::new(
            "viewer".to_string(),
            "source".to_string(),
            bad,
            good,
            ProjectionKind::View,
        );
        assert!(
            matches!(as_source, Err(ProjectionError::EmptySourceRect { .. })),
            "{bad:?} accepted as source_rect: {as_source:?}"
        );

        let as_region = Projection::new(
            "viewer".to_string(),
            "source".to_string(),
            good,
            bad,
            ProjectionKind::View,
        );
        assert!(
            matches!(as_region, Err(ProjectionError::EmptyRegion { .. })),
            "{bad:?} accepted as region: {as_region:?}"
        );
    }
}

#[test]
fn letterbox_of_an_empty_source_is_empty_not_nan() {
    for source in [size(0., 800.), size(1280., 0.), size(0., 0.)] {
        let r = letterbox(source, size(5120., 1440.));
        assert!(
            r.loc.x.is_finite() && r.loc.y.is_finite(),
            "letterbox({source:?}) gave a non-finite location: {r:?}"
        );
        assert_eq!(r.size, size(0., 0.), "letterbox({source:?})");
    }
}

#[test]
fn constructor_rejects_aspect_ratio_mismatch() {
    let err = Projection::new(
        "viewer".to_string(),
        "source".to_string(),
        rect(0., 0., 1728., 1080.),
        rect(0., 0., 2304., 1000.),
        ProjectionKind::View,
    )
    .unwrap_err();

    assert!(matches!(err, ProjectionError::AspectRatioMismatch { .. }));
}

#[test]
fn constructor_rejects_viewer_equals_source() {
    let err = Projection::new(
        "same".to_string(),
        "same".to_string(),
        rect(0., 0., 1728., 1080.),
        rect(0., 0., 2304., 1440.),
        ProjectionKind::View,
    )
    .unwrap_err();

    assert!(matches!(err, ProjectionError::ViewerIsSource { .. }));
}

#[test]
fn letterbox_wide_source_into_wide_viewer() {
    let region = letterbox(size(1920., 1080.), size(3840., 1080.));

    assert!((region.size.h - 1080.).abs() < 1e-9);
    assert!((region.size.w - 1920.).abs() < 1e-9);
    assert!((region.loc.x - 960.).abs() < 1e-9);
    assert!((region.loc.y - 0.).abs() < 1e-9);
}

#[test]
fn letterbox_wide_source_into_tall_viewer() {
    let region = letterbox(size(1920., 1080.), size(1080., 1920.));

    assert!((region.size.w - 1080.).abs() < 1e-9);
    assert!((region.size.h - 607.5).abs() < 1e-9);
    assert!((region.loc.x - 0.).abs() < 1e-9);
    assert!((region.loc.y - (1920. - 607.5) / 2.).abs() < 1e-9);
}

#[test]
fn letterbox_tall_source_into_wide_viewer() {
    let region = letterbox(size(1080., 1920.), size(1920., 1080.));

    assert!((region.size.h - 1080.).abs() < 1e-9);
    assert!((region.size.w - 607.5).abs() < 1e-9);
    assert!((region.loc.y - 0.).abs() < 1e-9);
    assert!((region.loc.x - (1920. - 607.5) / 2.).abs() < 1e-9);
}

#[test]
fn overview_columns_zero_sources_is_empty() {
    let strip = rect(600., 0., 800., 1000.);

    assert!(overview_columns(strip, size(2000., 1000.), &[]).is_empty());
}

#[test]
fn overview_columns_one_source_starts_one_gap_right_of_the_strip() {
    let viewer_size = size(3000., 1000.);
    let strip = rect(1000., 0., 1000., 1000.);

    let regions = overview_columns(strip, viewer_size, &[size(800., 600.)]);

    // gap = 0.05 * 1000. The strip is not moved; the region sits at scale 1,
    // centred vertically.
    assert_eq!(regions, vec![rect(2050., 200., 800., 600.)]);
}

#[test]
fn overview_columns_three_sources_that_fit_keep_scale_one() {
    let viewer_size = size(5120., 1440.);
    let strip = rect(2000., 0., 800., 1440.);
    let sources = [size(640., 400.), size(960., 540.), size(200., 100.)];

    let regions = overview_columns(strip, viewer_size, &sources);

    // gap = 72. Room = 5120 - 2800 - 3 * 72 - 72 = 2032 >= 1800.
    assert_eq!(
        regions,
        vec![
            rect(2872., 520., 640., 400.),
            rect(3584., 450., 960., 540.),
            rect(4616., 670., 200., 100.),
        ]
    );
}

#[test]
fn overview_columns_overflow_shrinks_only_sources_to_fit() {
    let viewer_size = size(2000., 1000.);
    let strip = rect(500., 0., 500., 1000.);
    let sources = [size(600., 600.), size(600., 600.), size(600., 600.)];

    let regions = overview_columns(strip, viewer_size, &sources);

    // gap = 50. Room = 2000 - 1000 (strip right) - 3 * 50 - 50 (margin) = 800,
    // so each 600-wide source shrinks to 800 / 3.
    assert_eq!(regions.len(), 3);
    let w = 800. / 3.;
    let mut x = 1050.;
    for r in &regions {
        assert!((r.loc.x - x).abs() < 1e-9, "{regions:?}");
        assert!((r.size.w - w).abs() < 1e-9, "{regions:?}");
        assert!((r.size.h - w).abs() < 1e-9, "{regions:?}");
        assert!((r.loc.y - (1000. - w) / 2.).abs() < 1e-9, "{regions:?}");
        x += w + 50.;
    }
    let last = regions[2];
    assert!(
        (last.loc.x + last.size.w - 1950.).abs() < 1e-9,
        "{regions:?}"
    );
}

#[test]
fn overview_columns_shrink_stops_at_minimum_scale() {
    let viewer_size = size(2000., 1000.);
    let strip = rect(10., 0., 1980., 1000.);

    let regions = overview_columns(strip, viewer_size, &[size(600., 400.)]);

    // No room at all, so the source is clamped to scale 0.05.
    assert_eq!(regions, vec![rect(2040., 490., 30., 20.)]);
}

#[test]
fn overview_columns_zoom_one_places_regions_off_screen() {
    let viewer_size = size(2000., 1000.);
    let strip = rect(0., 0., 2000., 1000.);
    let sources = [size(400., 300.), size(200., 100.)];

    let regions = overview_columns(strip, viewer_size, &sources);

    assert_eq!(regions.len(), 2);
    for (r, source) in regions.iter().zip(&sources) {
        assert!(r.loc.x >= viewer_size.w, "{regions:?}");
        assert_eq!(r.size, *source);
    }
}

mod fixture_tests {
    use smithay::utils::{Logical, Point};

    use crate::niri::LockState;
    use crate::projection::{ProjectionKind, Viewing};
    use crate::tests::fixture::Fixture;

    #[test]
    fn output_under_passes_through_with_no_projections() {
        let mut f = Fixture::new();
        f.add_output(1, (1920, 1080));

        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();

        let physical = f.niri_output(1);
        let point = Point::<f64, Logical>::from((100., 50.));

        let (output, local) = f.niri().output_under(point).unwrap();
        assert_eq!(output, &physical);
        assert_eq!(local, point);
    }

    #[test]
    fn rebuild_projections_builds_view_projection_when_viewing_and_overview_closed() {
        let mut f = Fixture::new();
        f.add_output(1, (1920, 1080));

        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();

        let viewer_name = f.niri_output(1).name();
        f.niri().projection_state.viewing = Some(Viewing {
            viewer: viewer_name.clone(),
            source: "steam".to_string(),
        });
        f.niri().rebuild_projections();

        let projections = &f.niri().projection_state.projections;
        assert_eq!(projections.len(), 1);
        let projection = &projections[0];
        assert_eq!(projection.kind, ProjectionKind::View);
        assert_eq!(projection.viewer, viewer_name);
        assert_eq!(projection.source, "steam");

        // The 1280x800 source letterboxed into the 1920x1080 viewer scales
        // by 1.35 (bound by height) and is offset 96px horizontally, so the
        // viewer's centre maps back to the source's centre.
        let source_point = projection.to_source(Point::from((960., 540.))).unwrap();
        assert!((source_point.x - 640.).abs() < 1e-6);
        assert!((source_point.y - 400.).abs() < 1e-6);
    }

    #[test]
    fn rebuild_projections_clears_viewing_when_source_removed() {
        let mut f = Fixture::new();
        f.add_output(1, (1920, 1080));

        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();

        let viewer_name = f.niri_output(1).name();
        f.niri().projection_state.viewing = Some(Viewing {
            viewer: viewer_name.clone(),
            source: "steam".to_string(),
        });
        f.niri().rebuild_projections();
        assert!(f.niri().projection_state.viewing.is_some());

        let state = f.niri_state();
        state
            .backend
            .headless()
            .remove_virtual_output(&mut state.niri, "steam")
            .unwrap();
        state.niri.rebuild_projections();

        assert_eq!(f.niri().projection_state.viewing, None);
        assert!(f.niri().projection_state.projections.is_empty());
    }

    #[test]
    fn output_under_resolves_into_the_source_while_viewing() {
        let mut f = Fixture::new();
        f.add_output(1, (1920, 1080));

        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();

        let viewer = f.niri_output(1);
        let viewer_name = viewer.name();
        f.niri().projection_state.viewing = Some(Viewing {
            viewer: viewer_name.clone(),
            source: "steam".to_string(),
        });
        f.niri().rebuild_projections();

        let steam = f
            .niri()
            .layout
            .outputs()
            .find(|o| o.name() == "steam")
            .unwrap()
            .clone();

        // Viewer centre, per the letterbox computed in the test above.
        let (output, local) = f.niri().output_under(Point::from((960., 540.))).unwrap();
        assert_eq!(output, &steam);
        assert!((local.x - 640.).abs() < 1e-6);
        assert!((local.y - 400.).abs() < 1e-6);
    }

    #[test]
    fn output_under_returns_none_in_the_letterbox_bar_while_viewing() {
        let mut f = Fixture::new();
        f.add_output(1, (1920, 1080));

        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();

        let viewer_name = f.niri_output(1).name();
        f.niri().projection_state.viewing = Some(Viewing {
            viewer: viewer_name,
            source: "steam".to_string(),
        });
        f.niri().rebuild_projections();

        // Region is x in [96, 1824], so x=10 is in the left letterbox bar.
        assert_eq!(f.niri().output_under(Point::from((10., 540.))), None);
    }

    #[test]
    fn hot_corner_follows_the_viewer_not_the_projected_source() {
        let mut f = Fixture::new();
        f.add_output(1, (1920, 1080));

        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();

        let viewer = f.niri_output(1);
        f.niri().projection_state.viewing = Some(Viewing {
            viewer: viewer.name(),
            source: "steam".to_string(),
        });
        f.niri().rebuild_projections();

        // The viewer's own top-left corner lies in the letterbox bar.
        let viewer_corner = f.niri().contents_under(Point::from((0.5, 0.5)));
        assert!(viewer_corner.hot_corner);

        // The region starts at x=96, so this maps onto the source's top-left
        // corner, which is nowhere near a corner of the viewer.
        let source_corner = f.niri().contents_under(Point::from((96.2, 0.2)));
        assert!(!source_corner.hot_corner);
    }

    #[test]
    fn locking_the_session_stops_the_pointer_reaching_the_source() {
        let mut f = Fixture::new();
        f.add_output(1, (1920, 1080));

        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();

        let viewer = f.niri_output(1);
        f.niri().projection_state.viewing = Some(Viewing {
            viewer: viewer.name(),
            source: "steam".to_string(),
        });
        f.niri().rebuild_projections();
        let centre = Point::<f64, Logical>::from((960., 540.));
        assert_ne!(f.niri().output_under(centre).unwrap().0, &viewer);

        let id = f.add_client();
        let _lock = f.client(id).lock_session();
        f.roundtrip(id);
        assert!(!matches!(f.niri().lock_state, LockState::Unlocked));

        assert!(f.niri().projection_state.projections.is_empty());
        let (output, local) = f.niri().output_under(centre).unwrap();
        assert_eq!(output, &viewer);
        assert_eq!(local, centre);
        // Viewing is kept so the view comes back once the session unlocks.
        assert!(f.niri().projection_state.viewing.is_some());

        f.niri().unlock();
        assert_eq!(f.niri().projection_state.projections.len(), 1);
        assert_ne!(f.niri().output_under(centre).unwrap().0, &viewer);
    }
}

/// A raw `global_space.output_under` call skips projection resolution, so
/// every caller other than `Niri::output_under` itself must go through it
/// instead. This walks the source tree and fails if a new raw call sneaks
/// in anywhere but the two allowed sites.
///
/// The opposite mistake, placing, wrapping or clamping the real cursor with
/// a projected source's `global_space` geometry, has no textual signature to
/// grep for. `input_tests` below catches it behaviourally by driving the real
/// handlers and asserting the cursor stays on the viewer.
#[test]
fn global_space_output_under_is_used_only_in_niri_output_under_and_the_motion_clamp() {
    fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // Test code is allowed to mention the symbol in strings and
            // comments (as this guard test itself does); only production
            // code is required to route through `Niri::output_under`.
            if path.file_name().is_some_and(|name| name == "tests") {
                continue;
            }
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);
    assert!(
        !files.is_empty(),
        "expected to find .rs files under {src_dir:?}"
    );

    let mut hits = Vec::new();
    for path in files {
        let contents = std::fs::read_to_string(&path).unwrap_or_default();
        // Collapse whitespace so a method chain rustfmt wrapped across lines
        // (`.global_space\n.output_under(..)`) still matches.
        let stripped: String = contents.chars().filter(|c| !c.is_whitespace()).collect();
        let count = stripped.matches("global_space.output_under").count();
        if count > 0 {
            let rel = path
                .strip_prefix(&src_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            hits.push((rel, count));
        }
    }
    hits.sort();

    assert_eq!(
        hits,
        vec![("input/mod.rs".to_string(), 2), ("niri.rs".to_string(), 1)],
        "global_space.output_under must be used only inside Niri::resolve_output_under (src/niri.rs) \
         and the pointer-motion clamp (src/input/mod.rs); every other caller should go through \
         Niri::output_under so pointer positions are resolved through projections"
    );
}

mod overview_tests {
    use smithay::desktop::Window;
    use smithay::output::Output;
    use smithay::reexports::wayland_server::Resource as _;
    use smithay::utils::{Logical, Point, Rectangle};
    use smithay::wayland::seat::WaylandFocus as _;
    use wayland_client::protocol::wl_surface::WlSurface;
    use wayland_client::Proxy as _;

    use crate::layout::HitType;
    use crate::niri::Niri;
    use crate::projection::{Projection, ProjectionKind, Viewing};
    use crate::tests::client::ClientId;
    use crate::tests::fixture::Fixture;

    /// A wide physical viewer, like the 5120x1440 desk monitor, and the given
    /// virtual outputs.
    pub(super) fn set_up(viewer_size: (u16, u16), sources: &[(&str, u16, u16)]) -> Fixture {
        let mut f = Fixture::new();
        f.add_output(1, viewer_size);
        for (name, w, h) in sources {
            let state = f.niri_state();
            state
                .backend
                .headless()
                .create_virtual_output(&mut state.niri, *w, *h, 60, Some(name.to_string()))
                .unwrap();
        }
        f
    }

    pub(super) fn output_named(f: &mut Fixture, name: &str) -> Output {
        f.niri()
            .layout
            .outputs()
            .find(|o| o.name() == name)
            .unwrap()
            .clone()
    }

    pub(super) fn map_window_on(
        f: &mut Fixture,
        id: ClientId,
        output: &Output,
        w: u16,
        h: u16,
    ) -> Window {
        f.niri().layout.focus_output(output);

        let window = f.client(id).create_window();
        let surface = window.surface.clone();
        window.commit();
        f.roundtrip(id);

        let window = f.client(id).window(&surface);
        window.attach_new_buffer();
        window.set_size(w, h);
        window.ack_last_and_commit();
        f.double_roundtrip(id);

        window_for(f, &surface)
    }

    fn window_for(f: &mut Fixture, surface: &WlSurface) -> Window {
        let niri = f.niri();
        niri.layout
            .windows()
            .map(|(_, mapped)| mapped.window.clone())
            .find(|window| {
                window
                    .wl_surface()
                    .is_some_and(|s| s.id().protocol_id() == surface.id().protocol_id())
            })
            .unwrap()
    }

    fn center(rect: Rectangle<f64, Logical>) -> Point<f64, Logical> {
        rect.loc + rect.size.downscale(2.).to_point()
    }

    fn open_overview(f: &mut Fixture) {
        f.niri().layout.toggle_overview();
        f.niri_complete_animations();
        assert!(f.niri().layout.is_overview_open());
    }

    fn projection_for(niri: &Niri, source: &str) -> Projection {
        niri.projection_state
            .projections
            .iter()
            .find(|p| p.source == source && p.kind == ProjectionKind::Overview)
            .unwrap_or_else(|| {
                panic!(
                    "no overview projection for {source}: {:?}",
                    niri.projection_state.projections
                )
            })
            .clone()
    }

    fn viewer_origin(niri: &Niri, viewer: &Output) -> Point<f64, Logical> {
        niri.global_space
            .output_geometry(viewer)
            .unwrap()
            .loc
            .to_f64()
    }

    /// Where the centre of `window` is drawn on `output`, in output-local
    /// coordinates, following the overview zoom.
    pub(super) fn window_center_on(
        niri: &Niri,
        output: &Output,
        window: &Window,
    ) -> Point<f64, Logical> {
        let mon = niri.layout.monitor_for_output(output).unwrap();
        let zoom = mon.overview_zoom();
        let (ws, ws_geo) = mon
            .workspaces_with_render_geo_cull(false)
            .find(|(ws, _)| ws.has_window(window))
            .unwrap();
        let (tile, tile_pos, _) = ws
            .tiles_with_render_positions()
            .find(|(tile, _, _)| tile.window().window == *window)
            .unwrap();
        let center = tile_pos + tile.window_loc() + tile.window_size().to_point().downscale(2.);
        ws_geo.loc + center.upscale(zoom)
    }

    /// The global position on the viewer where `window` on `source` is shown.
    fn projected_window_center(
        f: &mut Fixture,
        source: &str,
        window: &Window,
    ) -> Point<f64, Logical> {
        let viewer = f.niri_output(1);
        let source_output = output_named(f, source);
        let niri = f.niri();
        let projection = projection_for(niri, source);
        let center = window_center_on(niri, &source_output, window);
        projection.to_viewer(center) + viewer_origin(niri, &viewer)
    }

    fn workspace_windows(niri: &Niri, output: &Output) -> Vec<Vec<Window>> {
        niri.layout
            .workspaces()
            .filter(|(mon, _, _)| mon.is_some_and(|mon| mon.output() == output))
            .map(|(_, _, ws)| ws.windows().map(|m| m.window.clone()).collect())
            .collect()
    }

    fn is_on(niri: &Niri, output: &Output, window: &Window) -> bool {
        niri.layout
            .windows_for_output(output)
            .any(|m| m.window == *window)
    }

    /// Drives an interactive move the way the pointer grab does: begin at
    /// `from`, move to `to`, release. Both are global positions resolved
    /// through `Niri::output_under`.
    fn drag(f: &mut Fixture, window: &Window, from: Point<f64, Logical>, to: Point<f64, Logical>) {
        let niri = f.niri();
        let (output, pos) = niri.output_under(from).unwrap();
        let output = output.clone();
        assert!(niri
            .layout
            .interactive_move_begin(window.clone(), &output, pos));

        let (output, pos) = niri.output_under(to).unwrap();
        let output = output.clone();
        let delta = to - from;
        assert!(niri
            .layout
            .interactive_move_update(window, delta, output.clone(), pos));
        assert!(niri
            .layout
            .interactive_move_update(window, Point::from((0., 0.)), output, pos));
        niri.layout.interactive_move_end(window);
        f.niri_complete_animations();
    }

    #[test]
    fn overview_hit_test_reaches_a_window_on_the_source() {
        let mut f = set_up((5120, 1440), &[("steam", 1280, 800)]);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let window = map_window_on(&mut f, id, &steam, 400, 300);
        open_overview(&mut f);

        let projection = projection_for(f.niri(), "steam");
        assert!((projection.scale() - 1.).abs() < 1e-9, "{projection:?}");

        let p = projected_window_center(&mut f, "steam", &window);
        let (output, _) = f.niri().output_under(p).unwrap();
        assert_eq!(output, &steam);

        let contents = f.niri().contents_under(p);
        let (hit_window, hit) = contents.window.unwrap();
        assert_eq!(hit_window, window);
        // The overview never delivers pointer input into windows (it only
        // activates them), for physical monitors too.
        assert!(matches!(hit, HitType::Activate { .. }), "{hit:?}");
    }

    #[test]
    fn overview_hit_test_reaches_a_window_in_a_shrunk_column() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let window = map_window_on(&mut f, id, &steam, 400, 300);
        open_overview(&mut f);

        let projection = projection_for(f.niri(), "steam");
        assert!(projection.scale() < 1., "{projection:?}");

        let p = projected_window_center(&mut f, "steam", &window);
        let contents = f.niri().contents_under(p);
        assert_eq!(contents.output.as_ref(), Some(&steam));
        assert_eq!(contents.window.map(|(w, _)| w), Some(window));
    }

    #[test]
    fn projected_surface_local_position_is_exact_under_scaling() {
        // A View projection scales steam by 1.35 onto the viewer; the
        // surface-local position smithay derives (pointer - focus location)
        // must still be the true position within the window's surface.
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let window = map_window_on(&mut f, id, &steam, 400, 300);
        let viewer = f.niri_output(1);

        f.niri().projection_state.viewing = Some(Viewing {
            viewer: viewer.name(),
            source: "steam".to_string(),
        });
        f.niri().rebuild_projections();

        let niri = f.niri();
        let projection = niri.projection_state.projections[0].clone();
        assert!((projection.scale() - 1.35).abs() < 1e-9);

        for offset in [(0., 0.), (-50., -40.), (37.5, 21.25)] {
            let source_pos = window_center_on(niri, &steam, &window) + Point::from(offset);
            let pointer = projection.to_viewer(source_pos) + viewer_origin(niri, &viewer);

            let Some((_, HitType::Input { win_pos })) =
                niri.layout.window_under(&steam, source_pos)
            else {
                panic!("no input hit on steam at {source_pos:?}");
            };
            let expected_local = source_pos - win_pos;

            let contents = niri.contents_under(pointer);
            let (surface, focus_loc) = contents.surface.unwrap();
            assert_eq!(Some(&surface), window.wl_surface().as_deref());
            let local = pointer - focus_loc;
            assert!(
                (local.x - expected_local.x).abs() <= 1.
                    && (local.y - expected_local.y).abs() <= 1.,
                "surface-local {local:?}, expected {expected_local:?} (pointer {pointer:?})"
            );
        }
    }

    #[test]
    fn drag_from_source_column_to_viewer_workspace() {
        let mut f = set_up((5120, 1440), &[("steam", 1280, 800)]);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let viewer = f.niri_output(1);
        let window = map_window_on(&mut f, id, &steam, 400, 300);
        open_overview(&mut f);

        let from = projected_window_center(&mut f, "steam", &window);
        let niri = f.niri();
        let mon = niri.layout.monitor_for_output(&viewer).unwrap();
        let (_, ws_geo) = mon.workspaces_with_render_geo().next().unwrap();
        let to = center(ws_geo) + viewer_origin(niri, &viewer);

        drag(&mut f, &window, from, to);

        assert!(is_on(f.niri(), &viewer, &window));
        assert!(!is_on(f.niri(), &steam, &window));
    }

    #[test]
    fn drag_from_viewer_workspace_into_source_column() {
        let mut f = set_up((5120, 1440), &[("steam", 1280, 800)]);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let viewer = f.niri_output(1);
        let window = map_window_on(&mut f, id, &viewer, 400, 300);
        open_overview(&mut f);

        let niri = f.niri();
        let from = window_center_on(niri, &viewer, &window) + viewer_origin(niri, &viewer);
        let projection = projection_for(niri, "steam");
        let mon = niri.layout.monitor_for_output(&steam).unwrap();
        let (_, ws_geo) = mon.workspaces_with_render_geo().next().unwrap();
        let to = projection.to_viewer(center(ws_geo)) + viewer_origin(niri, &viewer);

        drag(&mut f, &window, from, to);

        assert!(is_on(f.niri(), &steam, &window));
        assert!(!is_on(f.niri(), &viewer, &window));
    }

    #[test]
    fn drop_in_gap_between_source_workspaces_creates_a_workspace() {
        let mut f = set_up((5120, 1440), &[("steam", 1280, 800)]);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let viewer = f.niri_output(1);

        // steam: ws0 [a], ws1 [b], ws2 [] (the trailing empty one).
        let a = map_window_on(&mut f, id, &steam, 400, 300);
        let b = map_window_on(&mut f, id, &steam, 400, 300);
        f.niri().layout.move_to_workspace_down(false);
        f.double_roundtrip(id);
        let dragged = map_window_on(&mut f, id, &viewer, 400, 300);
        open_overview(&mut f);
        assert_eq!(
            workspace_windows(f.niri(), &steam),
            vec![vec![a.clone()], vec![b.clone()], vec![]]
        );

        let niri = f.niri();
        let from = window_center_on(niri, &viewer, &dragged) + viewer_origin(niri, &viewer);
        let projection = projection_for(niri, "steam");
        let mon = niri.layout.monitor_for_output(&steam).unwrap();
        let mut geos = mon.workspaces_render_geo();
        let ws0 = geos.next().unwrap();
        let ws1 = geos.next().unwrap();
        let gap_center = Point::from((center(ws0).x, (ws0.loc.y + ws0.size.h + ws1.loc.y) / 2.));
        let to = projection.to_viewer(gap_center) + viewer_origin(niri, &viewer);

        drag(&mut f, &dragged, from, to);

        assert_eq!(
            workspace_windows(f.niri(), &steam),
            vec![vec![a], vec![dragged], vec![b], vec![]]
        );
    }

    #[test]
    fn source_column_follows_source_removal_and_recreation_without_closing_overview() {
        let mut f = set_up((5120, 1440), &[("steam", 1280, 800)]);
        open_overview(&mut f);
        projection_for(f.niri(), "steam");

        let state = f.niri_state();
        state
            .backend
            .headless()
            .remove_virtual_output(&mut state.niri, "steam")
            .unwrap();
        assert!(f.niri().layout.is_overview_open());
        assert!(f.niri().projection_state.projections.is_empty());

        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();
        assert!(f.niri().layout.is_overview_open());
        projection_for(f.niri(), "steam");
    }

    #[test]
    fn reorder_windows_inside_the_source_column() {
        let mut f = set_up((5120, 1440), &[("steam", 1280, 800)]);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let a = map_window_on(&mut f, id, &steam, 300, 300);
        let b = map_window_on(&mut f, id, &steam, 300, 300);
        let viewer = f.niri_output(1);
        open_overview(&mut f);
        assert_eq!(
            workspace_windows(f.niri(), &steam)[0],
            vec![a.clone(), b.clone()]
        );

        let from = projected_window_center(&mut f, "steam", &a);
        let niri = f.niri();
        let projection = projection_for(niri, "steam");
        let mon = niri.layout.monitor_for_output(&steam).unwrap();
        let zoom = mon.overview_zoom();
        let (ws, ws_geo) = mon.workspaces_with_render_geo().next().unwrap();
        let (tile, tile_pos, _) = ws
            .tiles_with_render_positions()
            .find(|(tile, _, _)| tile.window().window == b)
            .unwrap();
        // Just past b's right edge, at b's vertical centre.
        let past_b = tile_pos + Point::from((tile.tile_size().w + 10., tile.tile_size().h / 2.));
        let past_b = ws_geo.loc + past_b.upscale(zoom);
        let to = projection.to_viewer(past_b) + viewer_origin(niri, &viewer);

        drag(&mut f, &a, from, to);

        assert_eq!(workspace_windows(f.niri(), &steam)[0], vec![b, a]);
    }

    #[test]
    fn scrolling_a_source_column_switches_its_workspaces_only() {
        let mut f = set_up((5120, 1440), &[("steam", 1280, 800)]);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let viewer = f.niri_output(1);
        map_window_on(&mut f, id, &steam, 300, 300);
        map_window_on(&mut f, id, &steam, 300, 300);
        f.niri().layout.move_to_workspace_down(false);
        f.double_roundtrip(id);
        open_overview(&mut f);

        let niri = f.niri();
        let steam_mon = niri.layout.monitor_for_output(&steam).unwrap();
        assert_eq!(steam_mon.active_workspace_idx(), 0);
        let viewer_idx = niri
            .layout
            .monitor_for_output(&viewer)
            .unwrap()
            .active_workspace_idx();

        // A point inside the lower (second) steam workspace in the column.
        let projection = projection_for(niri, "steam");
        let ws1 = steam_mon.workspaces_render_geo().nth(1).unwrap();
        let lower = Point::from((center(ws1).x, ws1.loc.y + 10.));
        let p = projection.to_viewer(lower) + viewer_origin(niri, &viewer);

        let (ws_output, ws) = niri.workspace_under(false, p).unwrap();
        assert_eq!(ws_output, steam);
        let ws_id = ws.id();
        let steam_ws1 = steam_mon
            .workspaces_with_render_geo()
            .nth(1)
            .unwrap()
            .0
            .id();
        assert_eq!(ws_id, steam_ws1);

        // The overview scroll handler begins the gesture on the output under
        // the cursor, then feeds it scroll deltas.
        let (output, _) = niri.output_under(p).unwrap();
        let output = output.clone();
        assert_eq!(output, steam);
        niri.layout.workspace_switch_gesture_begin(&output, true);
        niri.layout.workspace_switch_gesture_update(
            1000.,
            std::time::Duration::from_millis(10),
            true,
        );
        niri.layout.workspace_switch_gesture_end(Some(true));
        f.niri_complete_animations();

        let niri = f.niri();
        assert_ne!(
            niri.layout
                .monitor_for_output(&steam)
                .unwrap()
                .active_workspace_idx(),
            0
        );
        assert_eq!(
            niri.layout
                .monitor_for_output(&viewer)
                .unwrap()
                .active_workspace_idx(),
            viewer_idx
        );
    }

    #[test]
    fn every_enabled_source_gets_its_own_column() {
        let mut f = set_up((5120, 1440), &[("steam", 1280, 800), ("aux", 1920, 1080)]);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let aux = output_named(&mut f, "aux");
        let on_steam = map_window_on(&mut f, id, &steam, 300, 300);
        let on_aux = map_window_on(&mut f, id, &aux, 300, 300);
        open_overview(&mut f);

        let niri = f.niri();
        let steam_region = projection_for(niri, "steam").region;
        let aux_region = projection_for(niri, "aux").region;
        assert!(
            steam_region.intersection(aux_region).is_none(),
            "{steam_region:?} overlaps {aux_region:?}"
        );

        for (name, output, window) in [("steam", &steam, &on_steam), ("aux", &aux, &on_aux)] {
            let region = projection_for(f.niri(), name).region;
            let viewer = f.niri_output(1);
            let centre = center(region) + viewer_origin(f.niri(), &viewer);
            let (hit, _) = f.niri().output_under(centre).unwrap();
            assert_eq!(hit, output, "column centre of {name}");

            let p = projected_window_center(&mut f, name, window);
            let contents = f.niri().contents_under(p);
            assert_eq!(
                contents.window.map(|(w, _)| w).as_ref(),
                Some(window),
                "{name}"
            );
        }
    }

    #[test]
    fn redrawing_a_source_redraws_its_viewers_and_not_the_reverse() {
        let mut f = set_up((5120, 1440), &[("steam", 1280, 800)]);
        open_overview(&mut f);
        let viewer = f.niri_output(1);
        let steam = output_named(&mut f, "steam");

        let frames =
            |niri: &Niri, output: &Output| niri.output_state[output].frame_callback_sequence;

        let state = f.niri_state();
        state.niri.redraw_queued_outputs(&mut state.backend);
        let viewer_frames = frames(&state.niri, &viewer);
        let steam_frames = frames(&state.niri, &steam);

        state.niri.queue_redraw(&steam);
        state.niri.redraw_queued_outputs(&mut state.backend);
        assert_eq!(frames(&state.niri, &steam), steam_frames + 1);
        assert_eq!(frames(&state.niri, &viewer), viewer_frames + 1);

        state.niri.queue_redraw(&viewer);
        state.niri.redraw_queued_outputs(&mut state.backend);
        assert_eq!(frames(&state.niri, &steam), steam_frames + 1);
        assert_eq!(frames(&state.niri, &viewer), viewer_frames + 2);
    }
}

mod view_tests {
    use niri_config::Action;
    use niri_ipc::ViewOutputState;
    use smithay::output::Output;
    use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
    use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Anchor;
    use smithay::utils::{Logical, Point};

    use super::overview_tests::{map_window_on, output_named, set_up, window_center_on};
    use crate::backend::virtual_output::VirtualOutputError;
    use crate::layout::workspace::WorkspaceId;
    use crate::projection::{ProjectionKind, Viewing};
    use crate::tests::client::LayerConfigureProps;
    use crate::tests::fixture::Fixture;

    fn viewing(f: &mut Fixture) -> Option<Viewing> {
        f.niri().projection_state.viewing.clone()
    }

    fn active_output_name(f: &mut Fixture) -> String {
        f.niri().layout.active_output().unwrap().name()
    }

    fn start(f: &mut Fixture, name: &str) -> ViewOutputState {
        let state = f.niri().start_viewing(name).unwrap();
        f.niri_complete_animations();
        state
    }

    #[test]
    fn viewing_routes_the_viewer_centre_to_the_source_window() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let window = map_window_on(&mut f, id, &steam, 400, 300);
        f.niri().layout.focus_output(&viewer);

        let reply = start(&mut f, "steam");
        assert_eq!(
            reply,
            ViewOutputState::Viewing {
                viewer: viewer.name(),
                source: "steam".to_string(),
            }
        );
        assert_eq!(active_output_name(&mut f), "steam");

        let niri = f.niri();
        let projection = niri
            .projection_state
            .projections
            .iter()
            .find(|p| p.kind == ProjectionKind::View)
            .unwrap()
            .clone();
        let centre_on_source = window_center_on(niri, &steam, &window);
        let pos = projection.to_viewer(centre_on_source);

        let contents = niri.contents_under(pos);
        assert_eq!(contents.window.as_ref().unwrap().0, window);
        let (_, focus_loc) = contents.surface.unwrap();
        let surface_local = pos - focus_loc;
        // One physical pixel on the scale-1 viewer.
        assert!(
            (surface_local.x - 200.).abs() < 1. && (surface_local.y - 150.).abs() < 1.,
            "surface-local position {surface_local:?} is not the window centre"
        );

        // The letterbox is x in [96, 1824]; x=10 is in the left bar.
        let bar = niri.contents_under(Point::<f64, Logical>::from((10., 540.)));
        assert!(bar.surface.is_none());
        assert!(bar.window.is_none());
    }

    #[test]
    fn stop_returns_the_viewer_to_its_own_workspaces() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        start(&mut f, "steam");

        let reply = f.niri().stop_viewing();

        assert_eq!(
            reply,
            ViewOutputState::Stopped {
                viewer: viewer.name(),
                source: "steam".to_string(),
            }
        );
        assert_eq!(viewing(&mut f), None);
        assert!(f.niri().projection_state.projections.is_empty());
        assert_eq!(active_output_name(&mut f), viewer.name());
    }

    #[test]
    fn stop_when_not_viewing_changes_nothing() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let active = active_output_name(&mut f);

        assert_eq!(f.niri().stop_viewing(), ViewOutputState::NotViewing);
        assert_eq!(viewing(&mut f), None);
        assert_eq!(active_output_name(&mut f), active);
    }

    #[test]
    fn viewing_another_source_switches_on_the_same_viewer() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800), ("aux", 1920, 1080)]);
        let viewer = f.niri_output(1);
        start(&mut f, "steam");

        let reply = start(&mut f, "aux");

        assert_eq!(
            reply,
            ViewOutputState::Viewing {
                viewer: viewer.name(),
                source: "aux".to_string(),
            }
        );
        assert_eq!(active_output_name(&mut f), "aux");
    }

    #[test]
    fn removing_the_source_ends_view_mode_on_the_viewer() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        // A second physical output, so the viewer is not simply the only
        // output left to fall back to.
        f.add_output(2, (1920, 1080));
        let viewer = f.niri_output(2);
        f.niri().layout.focus_output(&viewer);
        start(&mut f, "steam");

        let state = f.niri_state();
        state
            .backend
            .headless()
            .remove_virtual_output(&mut state.niri, "steam")
            .unwrap();

        assert_eq!(viewing(&mut f), None);
        assert!(f.niri().projection_state.projections.is_empty());
        assert_eq!(active_output_name(&mut f), viewer.name());
    }

    #[test]
    fn removing_the_viewer_ends_view_mode_and_keeps_the_source() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        start(&mut f, "steam");

        f.niri().remove_output(&viewer);

        assert_eq!(viewing(&mut f), None);
        assert!(f.niri().projection_state.projections.is_empty());
        assert!(f.niri().layout.outputs().any(|o| o.name() == "steam"));
    }

    #[test]
    fn rejected_names_leave_view_mode_unchanged() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        start(&mut f, "steam");
        let before = viewing(&mut f);
        assert!(before.is_some());

        assert_eq!(
            f.niri().start_viewing("nope"),
            Err(VirtualOutputError::NotFound("nope".to_string()))
        );
        assert_eq!(viewing(&mut f), before);

        assert_eq!(
            f.niri().start_viewing(&viewer.name()),
            Err(VirtualOutputError::NotVirtual(viewer.name()))
        );
        assert_eq!(viewing(&mut f), before);
        assert_eq!(active_output_name(&mut f), "steam");
    }

    #[test]
    fn the_viewers_overlay_layer_stays_on_top_of_the_source_for_input() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        map_window_on(&mut f, id, &steam, 1280, 800);
        f.niri().layout.focus_output(&viewer);

        let layer = f.client(id).create_layer(None, Layer::Overlay, "osd");
        let surface = layer.surface.clone();
        layer.set_configure_props(LayerConfigureProps {
            anchor: Some(Anchor::Left | Anchor::Top),
            size: Some((200, 200)),
            ..Default::default()
        });
        layer.commit();
        f.roundtrip(id);
        let layer = f.client(id).layer(&surface);
        layer.attach_new_buffer();
        layer.set_size(200, 200);
        layer.ack_last_and_commit();
        f.double_roundtrip(id);

        start(&mut f, "steam");

        // Inside both the letterbox region (x >= 96) and the overlay surface.
        let contents = f.niri().contents_under(Point::from((150., 100.)));
        assert!(contents.layer.is_some(), "overlay surface lost the pointer");
        assert_eq!(contents.output.as_ref(), Some(&viewer));

        // Outside the overlay surface the source is hit as before.
        let contents = f.niri().contents_under(Point::from((960., 540.)));
        assert!(contents.layer.is_none());
        assert_eq!(contents.output.as_ref(), Some(&steam));
    }

    #[test]
    fn starting_shows_the_viewing_label_on_the_viewer_only() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);

        f.niri().start_viewing("steam").unwrap();

        let label = &f.niri().view_output_label;
        assert_eq!(label.text_on(&viewer.name()), Some("Viewing: steam"));
        assert_eq!(label.text_on("steam"), None);
    }

    #[test]
    fn losing_the_source_shows_why_on_the_viewer() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        start(&mut f, "steam");

        let state = f.niri_state();
        state
            .backend
            .headless()
            .remove_virtual_output(&mut state.niri, "steam")
            .unwrap();

        let text = f.niri().view_output_label.text_on(&viewer.name());
        assert_eq!(
            text,
            Some("Stopped viewing steam: source output no longer present")
        );
    }

    #[test]
    fn viewing_a_turned_off_virtual_output_says_it_is_off() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        // Turning a virtual output off takes it out of the layout while the
        // backend keeps listing it.
        let steam = output_named(&mut f, "steam");
        f.niri().remove_output(&steam);

        assert_eq!(
            f.niri_state().view_output(Some("steam")),
            Err(VirtualOutputError::Disabled("steam".to_string()))
        );
        assert_eq!(viewing(&mut f), None);
    }

    #[test]
    fn view_output_request_starts_and_stops() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1).name();

        assert_eq!(
            f.niri_state().view_output(Some("steam")),
            Ok(ViewOutputState::Viewing {
                viewer: viewer.clone(),
                source: "steam".to_string(),
            })
        );
        assert_eq!(
            f.niri_state().view_output(Some("nope")),
            Err(VirtualOutputError::NotFound("nope".to_string()))
        );
        assert_eq!(
            f.niri_state().view_output(None),
            Ok(ViewOutputState::Stopped {
                viewer,
                source: "steam".to_string(),
            })
        );
        assert_eq!(
            f.niri_state().view_output(None),
            Ok(ViewOutputState::NotViewing)
        );
    }

    #[test]
    fn view_output_bind_enters_view_mode_and_leaves_it() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1).name();

        f.niri_state()
            .do_action(Action::ViewOutput(Some("steam".to_string())), false);
        assert_eq!(
            viewing(&mut f),
            Some(Viewing {
                viewer: viewer.clone(),
                source: "steam".to_string(),
            })
        );

        f.niri_state().do_action(Action::ViewOutput(None), false);
        assert_eq!(viewing(&mut f), None);
        assert_eq!(active_output_name(&mut f), viewer);
    }

    #[test]
    fn a_rejected_view_output_bind_shows_the_error_on_the_screen() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1).name();

        f.niri_state()
            .do_action(Action::ViewOutput(Some("nope".to_string())), false);

        let expected = VirtualOutputError::NotFound("nope".to_string()).to_string();
        assert_eq!(
            f.niri().view_output_label.text_on(&viewer),
            Some(expected.as_str())
        );
        assert_eq!(viewing(&mut f), None);
    }

    fn open_overview(f: &mut Fixture) {
        f.niri().layout.toggle_overview();
        f.niri_complete_animations();
        assert!(f.niri().layout.is_overview_open());
    }

    /// The id of the workspace at `idx` on `output`, and whether it is active.
    fn workspace_on(f: &mut Fixture, output: &Output, idx: usize) -> (WorkspaceId, bool) {
        let (mon, _, ws) = f
            .niri()
            .layout
            .workspaces()
            .find(|(mon, i, _)| mon.is_some_and(|m| m.output() == output) && *i == idx)
            .unwrap();
        (ws.id(), mon.unwrap().active_workspace_idx() == idx)
    }

    #[test]
    fn clicking_a_source_workspace_in_the_overview_views_it() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        map_window_on(&mut f, id, &steam, 400, 300);
        f.niri().layout.focus_output(&viewer);
        open_overview(&mut f);
        // The empty workspace below the window's.
        let (ws_id, active) = workspace_on(&mut f, &steam, 1);
        assert!(!active);

        f.niri().activate_overview_workspace(&steam, ws_id, &viewer);
        f.niri_complete_animations();

        assert!(!f.niri().layout.is_overview_open());
        assert!(workspace_on(&mut f, &steam, 1).1);
        assert_eq!(
            viewing(&mut f),
            Some(Viewing {
                viewer: viewer.name(),
                source: "steam".to_string(),
            })
        );
        assert_eq!(active_output_name(&mut f), "steam");
    }

    #[test]
    fn clicking_a_viewer_workspace_in_the_overview_behaves_as_before() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        map_window_on(&mut f, id, &viewer, 400, 300);
        open_overview(&mut f);
        let (ws_id, active) = workspace_on(&mut f, &viewer, 1);
        assert!(!active);

        f.niri()
            .activate_overview_workspace(&viewer, ws_id, &viewer);
        f.niri_complete_animations();

        assert!(!f.niri().layout.is_overview_open());
        assert!(workspace_on(&mut f, &viewer, 1).1);
        assert_eq!(viewing(&mut f), None);
        assert_eq!(active_output_name(&mut f), viewer.name());
    }

    #[test]
    fn clicking_a_viewer_workspace_while_viewing_leaves_view_mode() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        map_window_on(&mut f, id, &viewer, 400, 300);
        start(&mut f, "steam");
        open_overview(&mut f);
        let (ws_id, _) = workspace_on(&mut f, &viewer, 1);

        f.niri()
            .activate_overview_workspace(&viewer, ws_id, &viewer);
        f.niri_complete_animations();

        assert_eq!(viewing(&mut f), None);
        assert!(workspace_on(&mut f, &viewer, 1).1);
        assert_eq!(active_output_name(&mut f), viewer.name());
    }

    #[test]
    fn the_overview_during_view_mode_is_the_normal_overview() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        start(&mut f, "steam");

        open_overview(&mut f);
        let kinds: Vec<_> = f
            .niri()
            .projection_state
            .projections
            .iter()
            .map(|p| p.kind)
            .collect();
        assert_eq!(kinds, vec![ProjectionKind::Overview]);
        assert!(viewing(&mut f).is_some());

        f.niri().layout.toggle_overview();
        f.niri_complete_animations();
        let kinds: Vec<_> = f
            .niri()
            .projection_state
            .projections
            .iter()
            .map(|p| p.kind)
            .collect();
        assert_eq!(kinds, vec![ProjectionKind::View]);
    }

    #[test]
    fn view_mode_takes_over_as_soon_as_the_overview_starts_closing() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        start(&mut f, "steam");
        open_overview(&mut f);

        // Close without finishing the zoom animation.
        f.niri().layout.toggle_overview();
        f.niri().rebuild_projections();
        assert!(f.niri().layout.overview_zoom() < 1.);

        let kinds: Vec<_> = f
            .niri()
            .projection_state
            .projections
            .iter()
            .map(|p| p.kind)
            .collect();
        assert_eq!(kinds, vec![ProjectionKind::View]);
    }

    #[test]
    fn viewing_without_a_physical_output_fails() {
        let mut f = Fixture::new();
        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();

        assert_eq!(
            f.niri().start_viewing("steam"),
            Err(VirtualOutputError::NoViewer("steam".to_string()))
        );
        assert_eq!(viewing(&mut f), None);
    }
}

mod render_tests {
    use smithay::backend::renderer::element::Element as _;
    use smithay::backend::renderer::gles::GlesRenderer;
    use smithay::output::Output;
    use smithay::utils::{Logical, Physical, Rectangle, Scale};

    use crate::niri::OutputRenderElements;
    use crate::projection::{Projection, ProjectionKind};
    use crate::render_helpers::{RenderCtx, RenderTarget};
    use crate::tests::fixture::Fixture;

    fn set_up(sources: &[(&str, u16, u16)]) -> Fixture {
        let mut f = Fixture::new();
        f.niri_state().backend.headless().add_renderer().unwrap();
        f.add_output(1, (5120, 1440));
        for (name, w, h) in sources {
            let state = f.niri_state();
            state
                .backend
                .headless()
                .create_virtual_output(&mut state.niri, *w, *h, 60, Some(name.to_string()))
                .unwrap();
        }

        let id = f.add_client();
        for (name, _, _) in sources {
            let output = f
                .niri()
                .layout
                .outputs()
                .find(|o| o.name() == *name)
                .unwrap()
                .clone();
            f.niri().layout.focus_output(&output);
            let window = f.client(id).create_window();
            let surface = window.surface.clone();
            window.commit();
            f.roundtrip(id);
            let window = f.client(id).window(&surface);
            window.attach_new_buffer();
            window.set_size(300, 200);
            window.ack_last_and_commit();
            f.double_roundtrip(id);
        }

        f.niri().layout.toggle_overview();
        f.niri_complete_animations();
        f
    }

    fn render(f: &mut Fixture, output: &Output) -> Vec<OutputRenderElements<GlesRenderer>> {
        f.niri().update_render_elements(None);
        let state = f.niri_state();
        let niri = &state.niri;
        state
            .backend
            .headless()
            .with_primary_renderer(|renderer| {
                let ctx = RenderCtx {
                    renderer,
                    target: RenderTarget::Output,
                    xray: None,
                };
                niri.render_to_vec(ctx, output, false)
            })
            .unwrap()
    }

    fn overview_projections(f: &mut Fixture) -> Vec<Projection> {
        let projections: Vec<_> = f
            .niri()
            .projection_state
            .projections
            .iter()
            .filter(|p| p.kind == ProjectionKind::Overview)
            .cloned()
            .collect();
        assert!(!projections.is_empty());
        projections
    }

    fn physical(rect: Rectangle<f64, Logical>, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        rect.to_physical_precise_round(scale)
    }

    #[test]
    fn overview_renders_each_source_inside_its_region() {
        let mut f = set_up(&[("steam", 1280, 800), ("aux", 1920, 1080)]);
        let viewer = f.niri_output(1);
        let scale = Scale::from(viewer.current_scale().fractional_scale());
        let projections = overview_projections(&mut f);
        let regions: Vec<_> = projections
            .iter()
            .map(|p| physical(p.region, scale))
            .collect();

        let elements = render(&mut f, &viewer);
        let projected: Vec<_> = elements
            .iter()
            .filter(|e| matches!(e, OutputRenderElements::Projected(_)))
            .map(|e| e.geometry(scale))
            .collect();

        for region in &regions {
            assert!(
                projected.iter().any(|geo| region.contains_rect(*geo)),
                "no projected element inside {region:?}: {projected:?}"
            );
        }
        for geo in &projected {
            assert!(
                regions.iter().any(|region| region.contains_rect(*geo)),
                "projected element {geo:?} outside every region {regions:?}"
            );
        }
    }

    #[test]
    fn overview_labels_each_column_with_its_source_name() {
        let mut f = set_up(&[("steam", 1280, 800), ("aux", 1920, 1080)]);
        let viewer = f.niri_output(1);
        let scale = Scale::from(viewer.current_scale().fractional_scale());
        let projections = overview_projections(&mut f);

        let elements = render(&mut f, &viewer);
        let textures: Vec<_> = elements
            .iter()
            .filter(|e| matches!(e, OutputRenderElements::Texture(_)))
            .map(|e| e.geometry(scale))
            .collect();

        for projection in &projections {
            let region = physical(projection.region, scale);
            assert!(
                textures.iter().any(|geo| {
                    geo.loc.y + geo.size.h <= region.loc.y
                        && geo.loc.x >= region.loc.x
                        && geo.loc.x + geo.size.w <= region.loc.x + region.size.w
                }),
                "no label above the {} column {region:?}: {textures:?}",
                projection.source
            );
        }
    }

    #[test]
    fn rendering_a_source_never_includes_projections() {
        let mut f = set_up(&[("steam", 1280, 800)]);
        let steam = f
            .niri()
            .layout
            .outputs()
            .find(|o| o.name() == "steam")
            .unwrap()
            .clone();

        let elements = render(&mut f, &steam);
        assert!(!elements
            .iter()
            .any(|e| matches!(e, OutputRenderElements::Projected(_))));
    }

    fn projected_geometries(
        f: &mut Fixture,
        viewer: &Output,
        update_only_the_viewer: bool,
    ) -> Vec<Rectangle<i32, Physical>> {
        let update = update_only_the_viewer.then(|| viewer.clone());
        f.niri().update_render_elements(update.as_ref());
        let scale = Scale::from(viewer.current_scale().fractional_scale());
        let state = f.niri_state();
        let niri = &state.niri;
        let elements = state
            .backend
            .headless()
            .with_primary_renderer(|renderer| {
                let ctx = RenderCtx {
                    renderer,
                    target: RenderTarget::Output,
                    xray: None,
                };
                niri.render_to_vec(ctx, viewer, false)
            })
            .unwrap();
        elements
            .iter()
            .filter(|e| matches!(e, OutputRenderElements::Projected(_)))
            .map(|e| e.geometry(scale))
            .collect()
    }

    /// A redraw updates only the output being drawn (`redraw` calls
    /// `update_render_elements(Some(output))`), so the viewer's redraw must bring the sources
    /// it projects up to date too, or their columns show stale contents.
    #[test]
    fn a_viewer_redraw_refreshes_the_sources_it_projects() {
        let mut f = set_up(&[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);

        let viewer_only = projected_geometries(&mut f, &viewer, true);
        let everything = projected_geometries(&mut f, &viewer, false);

        assert_eq!(viewer_only, everything);
    }

    /// The damage tracker keys elements by id, so one frame must not carry an id twice.
    pub(super) fn assert_unique_ids(elements: &[OutputRenderElements<GlesRenderer>]) {
        let mut seen = Vec::new();
        for element in elements {
            let id = element.id();
            assert!(
                !seen.contains(&id),
                "element id {id:?} appears twice in one frame"
            );
            seen.push(id);
        }
    }

    #[test]
    fn overview_frame_has_unique_element_ids_with_a_notification_showing() {
        let mut f = set_up(&[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        f.niri().config_error_notification.show();
        f.niri_complete_animations();

        let elements = render(&mut f, &viewer);

        assert_unique_ids(&elements);
    }
}

mod view_render_tests {
    use smithay::backend::renderer::element::Element as _;
    use smithay::backend::renderer::gles::GlesRenderer;
    use smithay::output::Output;
    use smithay::utils::{Point, Rectangle, Scale, Size};

    use super::overview_tests::map_window_on;
    use crate::niri::OutputRenderElements;
    use crate::projection::ProjectionKind;
    use crate::render_helpers::{RenderCtx, RenderTarget};
    use crate::tests::fixture::Fixture;

    /// A 5120x1440 viewer with a window of its own, viewing a 1280x800
    /// `steam` with a window on it.
    fn set_up() -> (Fixture, Output) {
        let mut f = Fixture::new();
        f.niri_state().backend.headless().add_renderer().unwrap();
        f.add_output(1, (5120, 1440));
        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();
        let viewer = f.niri_output(1);
        let steam = f
            .niri()
            .layout
            .outputs()
            .find(|o| o.name() == "steam")
            .unwrap()
            .clone();

        let id = f.add_client();
        map_window_on(&mut f, id, &steam, 300, 200);
        map_window_on(&mut f, id, &viewer, 300, 200);
        f.niri().layout.focus_output(&viewer);

        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();
        (f, viewer)
    }

    pub(super) fn render(
        f: &mut Fixture,
        output: &Output,
    ) -> Vec<OutputRenderElements<GlesRenderer>> {
        f.niri().update_render_elements(None);
        let state = f.niri_state();
        let niri = &state.niri;
        state
            .backend
            .headless()
            .with_primary_renderer(|renderer| {
                let ctx = RenderCtx {
                    renderer,
                    target: RenderTarget::Output,
                    xray: None,
                };
                niri.render_to_vec(ctx, output, false)
            })
            .unwrap()
    }

    #[test]
    fn view_mode_draws_the_source_letterboxed_over_a_black_backdrop() {
        let (mut f, viewer) = set_up();
        let scale = Scale::from(viewer.current_scale().fractional_scale());
        let region = f
            .niri()
            .projection_state
            .projections
            .iter()
            .find(|p| p.kind == ProjectionKind::View)
            .unwrap()
            .region
            .to_physical_precise_round(scale);

        let elements = render(&mut f, &viewer);

        let projected: Vec<_> = elements
            .iter()
            .filter(|e| matches!(e, OutputRenderElements::Projected(_)))
            .map(|e| e.geometry(scale))
            .collect();
        assert!(!projected.is_empty(), "no projected elements");
        for geo in &projected {
            assert!(
                region.contains_rect(*geo),
                "projected element {geo:?} outside the letterbox {region:?}"
            );
        }

        let full = Rectangle::new(Point::from((0, 0)), Size::from((5120, 1440)));
        let black_backdrop = elements.iter().any(|e| match e {
            OutputRenderElements::SolidColor(solid) => {
                solid.geometry(scale) == full && solid.color().components() == [0., 0., 0., 1.]
            }
            _ => false,
        });
        assert!(black_backdrop, "no black backdrop covering the viewer");

        assert!(
            !elements.iter().any(|e| matches!(
                e,
                OutputRenderElements::Monitor(_) | OutputRenderElements::RelocatedColor(_)
            )),
            "the viewer's own workspaces are still drawn"
        );
    }

    #[test]
    fn view_mode_draws_the_viewing_label_on_the_viewer() {
        let (mut f, viewer) = set_up();

        let elements = render(&mut f, &viewer);

        assert!(elements
            .iter()
            .any(|e| matches!(e, OutputRenderElements::Texture(_))));
    }

    #[test]
    fn view_mode_frame_has_unique_element_ids_with_a_notification_showing() {
        let (mut f, viewer) = set_up();
        f.niri().config_error_notification.show();
        f.niri_complete_animations();

        let elements = render(&mut f, &viewer);

        super::render_tests::assert_unique_ids(&elements);
    }
}

/// The Alt-Tab switcher while a virtual output is viewed: it must be drawn on, and take
/// pointer input from, the physical viewer rather than the source nobody sees.
mod mru_tests {
    use niri_config::{Action, Config, MruDirection};
    use smithay::desktop::Window;
    use smithay::output::Output;
    use smithay::utils::{Logical, Point, Rectangle};

    use super::overview_tests::{map_window_on, output_named};
    use super::view_render_tests::render;
    use crate::niri::OutputRenderElements;
    use crate::tests::fixture::Fixture;
    use crate::tests::input;
    use crate::ui::mru::WindowMruUiRenderElement;

    /// A 1920x1080 viewer with a window of its own, viewing a 1280x800 `steam` with two
    /// windows, and the Alt-Tab switcher opened by its bind's action.
    fn set_up() -> (Fixture, Output, Vec<Window>) {
        let mut config = Config::default();
        // Show the switcher at once instead of after the default delay.
        config.recent_windows.open_delay_ms = 0;
        let mut f = Fixture::with_config(config);
        f.niri_state().backend.headless().add_renderer().unwrap();
        f.add_output(1, (1920, 1080));
        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();
        let viewer = f.niri_output(1);
        let steam = output_named(&mut f, "steam");

        let id = f.add_client();
        let mut windows = vec![map_window_on(&mut f, id, &viewer, 300, 200)];
        for _ in 0..2 {
            windows.push(map_window_on(&mut f, id, &steam, 300, 200));
        }
        f.niri().layout.focus_output(&viewer);
        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();
        assert_eq!(f.niri().layout.active_output(), Some(&steam));

        f.niri_state().do_action(
            Action::MruAdvance {
                direction: MruDirection::Forward,
                scope: None,
                filter: None,
            },
            false,
        );
        f.niri_complete_animations();
        assert!(f.niri().window_mru_ui.is_open());
        (f, viewer, windows)
    }

    fn center(rect: Rectangle<f64, Logical>) -> Point<f64, Logical> {
        rect.loc + rect.size.downscale(2.).to_point()
    }

    #[test]
    fn egl_alt_tab_in_view_mode_draws_the_thumbnails_on_the_viewer() {
        let (mut f, viewer, _) = set_up();

        let elements = render(&mut f, &viewer);

        let thumbnails = elements
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    OutputRenderElements::WindowMruUi(WindowMruUiRenderElement::Thumbnail(_))
                )
            })
            .count();
        assert_eq!(
            thumbnails,
            3,
            "the viewer's frame should show a thumbnail per window, MRU is on {:?}",
            f.niri().window_mru_ui.output().map(|o| o.name())
        );
    }

    #[test]
    fn egl_clicking_a_thumbnail_in_view_mode_activates_its_window() {
        let (mut f, viewer, windows) = set_up();
        assert_eq!(
            f.niri().window_mru_ui.output(),
            Some(&viewer),
            "the switcher opened on the source nobody sees"
        );
        let current = f.niri().window_mru_ui.current_window_id();
        let (target_id, rect) = f
            .niri()
            .window_mru_ui
            .thumbnails_in_view()
            .into_iter()
            .find(|(id, _)| Some(*id) != current)
            .unwrap();
        let target = windows
            .iter()
            .find(|w| f.niri().find_window_by_id(target_id).as_ref() == Some(*w))
            .unwrap()
            .clone();
        let viewer_loc = f
            .niri()
            .global_space
            .output_geometry(&viewer)
            .unwrap()
            .loc
            .to_f64();

        f.niri_state().move_cursor(center(rect) + viewer_loc);
        input::pointer_button(&mut f, input::BTN_LEFT, true);
        input::pointer_button(&mut f, input::BTN_LEFT, false);

        assert!(!f.niri().window_mru_ui.is_open());
        let focused = f.niri().layout.focus().map(|m| m.window.clone());
        assert_eq!(focused.as_ref(), Some(&target));
    }
}

/// Input driven through the real handlers (`do_action`, `process_input_event`) while a
/// projection is up. The real cursor must stay on the physical output it is on: a virtual
/// output's global-space geometry never places, wraps, clamps or offsets it.
mod input_tests {
    use niri_config::input::{WarpMouseToFocus, WarpMouseToFocusMode};
    use niri_config::{Action, Config};
    use smithay::desktop::Window;
    use smithay::input::pointer::{Focus, GrabStartData as PointerGrabStartData};
    use smithay::output::Output;
    use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
    use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Anchor;
    use smithay::utils::{Logical, Point, Rectangle, SERIAL_COUNTER};
    use wayland_client::protocol::wl_surface::WlSurface;

    use super::overview_tests::{map_window_on, output_named, window_center_on};
    use crate::input::move_grab::MoveGrab;
    use crate::input::AnyStartData;
    use crate::layout::HitType;
    use crate::projection::{Projection, ProjectionKind};
    use crate::tests::client::{ClientId, LayerConfigureProps};
    use crate::tests::fixture::Fixture;
    use crate::tests::input;

    fn set_up_with(config: Config, viewer: (u16, u16), sources: &[(&str, u16, u16)]) -> Fixture {
        let mut f = Fixture::with_config(config);
        f.add_output(1, viewer);
        for (name, w, h) in sources {
            let state = f.niri_state();
            state
                .backend
                .headless()
                .create_virtual_output(&mut state.niri, *w, *h, 60, Some(name.to_string()))
                .unwrap();
        }
        f
    }

    fn geometry(f: &mut Fixture, output: &Output) -> Rectangle<f64, Logical> {
        f.niri()
            .global_space
            .output_geometry(output)
            .unwrap()
            .to_f64()
    }

    fn view_projection(f: &mut Fixture) -> Projection {
        f.niri()
            .projection_state
            .projections
            .iter()
            .find(|p| p.kind == ProjectionKind::View)
            .unwrap()
            .clone()
    }

    fn pointer(f: &mut Fixture) -> Point<f64, Logical> {
        f.niri().seat.get_pointer().unwrap().current_location()
    }

    fn warp_config() -> Config {
        let mut config = Config::default();
        config.input.warp_mouse_to_focus = Some(WarpMouseToFocus {
            mode: Some(WarpMouseToFocusMode::CenterXyAlways),
        });
        config
    }

    #[test]
    fn warp_to_focus_in_view_mode_lands_on_the_viewer_over_the_focused_window() {
        let mut f = set_up_with(warp_config(), (1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let left = map_window_on(&mut f, id, &steam, 300, 300);
        map_window_on(&mut f, id, &steam, 300, 300);
        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();
        f.niri_state().move_cursor(Point::from((960., 540.)));
        f.niri_state().update_keyboard_focus();

        f.niri_state().do_action(Action::FocusColumnLeft, false);
        f.niri_complete_animations();

        let viewer_geo = geometry(&mut f, &viewer);
        let p = pointer(&mut f);
        assert!(
            viewer_geo.contains(p),
            "pointer {p:?} left the viewer {viewer_geo:?}"
        );
        let projection = view_projection(&mut f);
        let expected =
            projection.to_viewer(window_center_on(f.niri(), &steam, &left)) + viewer_geo.loc;
        assert!(
            (p.x - expected.x).abs() <= 1. && (p.y - expected.y).abs() <= 1.,
            "pointer {p:?} is not over the focused window's centre {expected:?}"
        );
    }

    #[test]
    fn warp_to_focus_on_an_unprojected_virtual_output_leaves_the_pointer() {
        let mut f = set_up_with(warp_config(), (1920, 1080), &[("steam", 1280, 800)]);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        map_window_on(&mut f, id, &steam, 300, 300);
        map_window_on(&mut f, id, &steam, 300, 300);
        f.niri_state().move_cursor(Point::from((960., 540.)));
        f.niri().layout.focus_output(&steam);
        f.niri_state().update_keyboard_focus();

        f.niri_state().do_action(Action::FocusColumnLeft, false);

        assert_eq!(pointer(&mut f), Point::from((960., 540.)));
    }

    fn open_overview(f: &mut Fixture) {
        f.niri().layout.toggle_overview();
        f.niri_complete_animations();
        assert!(f.niri().layout.is_overview_open());
    }

    fn overview_projection(f: &mut Fixture, source: &str) -> Projection {
        f.niri()
            .projection_state
            .projections
            .iter()
            .find(|p| p.kind == ProjectionKind::Overview && p.source == source)
            .unwrap()
            .clone()
    }

    fn center(rect: Rectangle<f64, Logical>) -> Point<f64, Logical> {
        rect.loc + rect.size.downscale(2.).to_point()
    }

    #[test]
    fn right_drag_in_a_source_column_keeps_the_pointer_on_the_viewer() {
        let mut f = set_up_with(Config::default(), (5120, 1440), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        open_overview(&mut f);
        let viewer_geo = geometry(&mut f, &viewer);
        let start = center(overview_projection(&mut f, "steam").region) + viewer_geo.loc;
        f.niri_state().move_cursor(start);

        input::pointer_button(&mut f, input::BTN_RIGHT, true);
        for _ in 0..3 {
            input::pointer_motion(&mut f, (20., 5.));
        }

        let p = pointer(&mut f);
        let expected = start + Point::from((60., 15.));
        assert!(
            viewer_geo.contains(p)
                && (p - expected).x.abs() < 1e-6
                && (p - expected).y.abs() < 1e-6,
            "pointer {p:?} should have moved to {expected:?} on the viewer {viewer_geo:?}"
        );
        input::pointer_button(&mut f, input::BTN_RIGHT, false);
    }

    fn pending_width(window: &Window) -> i32 {
        window
            .toplevel()
            .unwrap()
            .with_pending_state(|state| state.size)
            .unwrap()
            .w
    }

    #[test]
    fn mod_right_drag_resize_in_view_mode_follows_the_pointer_at_projection_scale() {
        let mut f = set_up_with(Config::default(), (1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let window = map_window_on(&mut f, id, &steam, 400, 300);
        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();
        let projection = view_projection(&mut f);
        assert!((projection.scale() - 1.35).abs() < 1e-9, "{projection:?}");

        let width_before = window.geometry().size.w;
        // Inside the right third of the window, so the drag moves its right edge.
        let grip = window_center_on(f.niri(), &steam, &window)
            + Point::from((f64::from(width_before) * 0.4, 0.));
        let viewer_geo = geometry(&mut f, &viewer);
        f.niri_state()
            .move_cursor(projection.to_viewer(grip) + viewer_geo.loc);

        input::super_key(&mut f, true);
        input::pointer_button(&mut f, input::BTN_RIGHT, true);
        // 135 viewer pixels are 100 source pixels at scale 1.35.
        for _ in 0..3 {
            input::pointer_motion(&mut f, (45., 0.));
        }
        input::pointer_button(&mut f, input::BTN_RIGHT, false);
        input::super_key(&mut f, false);
        f.double_roundtrip(id);

        let grown = pending_width(&window) - width_before;
        assert!(
            (grown - 100).abs() <= 1,
            "a 135 px drag at scale 1.35 grew the window by {grown} px, expected 100"
        );
    }

    #[test]
    fn tapping_a_source_workspace_views_it_on_the_tapped_monitor_not_the_mouses() {
        let mut f = set_up_with(Config::default(), (1920, 1080), &[("steam", 1280, 800)]);
        f.add_output(2, (1920, 1080));
        let tapped = f.niri_output(1);
        let with_mouse = f.niri_output(2);
        input::add_device(&mut f);
        f.niri().config.borrow_mut().input.touch.map_to_output = Some(tapped.name());
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        map_window_on(&mut f, id, &steam, 400, 300);
        open_overview(&mut f);
        let mouse_geo = geometry(&mut f, &with_mouse);
        f.niri_state().move_cursor(center(mouse_geo));

        let niri = f.niri();
        let projection = niri
            .projection_state
            .projections
            .iter()
            .find(|p| p.source == "steam" && p.viewer == tapped.name())
            .unwrap()
            .clone();
        // The empty workspace below the window's.
        let ws1 = niri
            .layout
            .monitor_for_output(&steam)
            .unwrap()
            .workspaces_render_geo()
            .nth(1)
            .unwrap();
        // Its top edge; the column crops the rest of it.
        let tap = projection.to_viewer(Point::from((center(ws1).x, ws1.loc.y + 40.)));
        let tapped_geo = geometry(&mut f, &tapped);
        input::touch_tap(
            &mut f,
            (tap.x / tapped_geo.size.w, tap.y / tapped_geo.size.h),
        );
        f.niri_complete_animations();

        let viewing = f.niri().projection_state.viewing.clone().unwrap();
        assert_eq!(viewing.source, "steam");
        assert_eq!(
            viewing.viewer,
            tapped.name(),
            "view mode started on the monitor under the mouse, not the one tapped"
        );
    }

    fn steam_view_pos(f: &mut Fixture, steam: &Output) -> f64 {
        f.niri()
            .layout
            .monitor_for_output(steam)
            .unwrap()
            .active_workspace_ref()
            .scrolling()
            .view_pos()
    }

    #[test]
    fn mod_middle_drag_in_view_mode_scrolls_the_source_at_projection_scale() {
        let mut f = set_up_with(Config::default(), (1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        for _ in 0..3 {
            map_window_on(&mut f, id, &steam, 1000, 300);
        }
        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();
        let projection = view_projection(&mut f);
        let viewer_geo = geometry(&mut f, &viewer);
        f.niri_state()
            .move_cursor(center(projection.region) + viewer_geo.loc);
        let view_pos_before = steam_view_pos(&mut f, &steam);

        input::super_key(&mut f, true);
        input::pointer_button(&mut f, input::BTN_MIDDLE, true);
        // 135 viewer pixels are 100 source pixels at scale 1.35.
        for _ in 0..3 {
            input::pointer_motion(&mut f, (45., 0.));
        }
        let scrolled = steam_view_pos(&mut f, &steam) - view_pos_before;
        input::pointer_button(&mut f, input::BTN_MIDDLE, false);
        input::super_key(&mut f, false);

        assert!(
            (scrolled + 100.).abs() < 1e-6,
            "a 135 px drag at scale 1.35 scrolled steam by {scrolled} px, expected -100"
        );
    }

    /// A client's titlebar drag (xdg_toplevel.move) starts a `MoveGrab` with view offset
    /// enabled. The test client has no seat to send that request, so this installs the grab
    /// the way the xdg-shell handler does and drives it with real pointer motion.
    #[test]
    fn horizontal_titlebar_drag_in_view_mode_keeps_the_pointer_on_the_viewer() {
        let mut f = set_up_with(Config::default(), (1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let window = map_window_on(&mut f, id, &steam, 400, 300);
        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();
        let viewer_geo = geometry(&mut f, &viewer);
        let start = view_projection(&mut f).to_viewer(window_center_on(f.niri(), &steam, &window))
            + viewer_geo.loc;
        f.niri_state().move_cursor(start);

        let start_data = AnyStartData::Pointer(PointerGrabStartData {
            focus: None,
            button: input::BTN_LEFT,
            location: start,
        });
        let grab = MoveGrab::new(f.niri_state(), start_data, window, true, None).unwrap();
        let pointer_handle = f.niri().seat.get_pointer().unwrap();
        pointer_handle.set_grab(
            f.niri_state(),
            grab,
            SERIAL_COUNTER.next_serial(),
            Focus::Clear,
        );
        for _ in 0..3 {
            input::pointer_motion(&mut f, (20., 1.));
        }

        let p = pointer(&mut f);
        let expected = start + Point::from((60., 3.));
        assert!(
            viewer_geo.contains(p)
                && (p - expected).x.abs() < 1e-6
                && (p - expected).y.abs() < 1e-6,
            "pointer {p:?} should have moved to {expected:?} on the viewer {viewer_geo:?}"
        );
    }

    #[test]
    fn the_viewers_top_layer_takes_input_over_overview_columns() {
        let mut f = set_up_with(Config::default(), (5120, 1440), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        map_window_on(&mut f, id, &steam, 1280, 800);
        f.niri().layout.focus_output(&viewer);

        // A bar down the viewer's right side, covering where the steam column is drawn.
        let layer = f.client(id).create_layer(None, Layer::Top, "bar");
        let surface = layer.surface.clone();
        layer.set_configure_props(LayerConfigureProps {
            anchor: Some(Anchor::Right | Anchor::Top | Anchor::Bottom),
            size: Some((3000, 0)),
            ..Default::default()
        });
        layer.commit();
        f.roundtrip(id);
        let layer = f.client(id).layer(&surface);
        layer.attach_new_buffer();
        layer.set_size(3000, 1440);
        layer.ack_last_and_commit();
        f.double_roundtrip(id);

        open_overview(&mut f);
        let viewer_geo = geometry(&mut f, &viewer);
        let column = overview_projection(&mut f, "steam").region;
        assert!(
            column.loc.x >= 5120. - 3000.,
            "the bar does not cover the column {column:?}"
        );
        f.niri_state().move_cursor(center(column) + viewer_geo.loc);
        input::pointer_motion(&mut f, (1., 0.));

        let contents = &f.niri().pointer_contents;
        assert!(
            contents.layer.is_some(),
            "the top-layer bar drawn above the column lost the pointer to {:?}",
            contents.output.as_ref().map(|o| o.name())
        );
        assert_eq!(contents.output.as_ref(), Some(&viewer));
    }

    /// View mode on a 1920x1080 viewer showing steam at scale 1.35, a 400x300 window on
    /// steam, and the pointer over it at `surface_local`. Returns the window's client surface
    /// and a mapping from surface-local positions to global pointer positions.
    fn view_window_under_pointer(
        f: &mut Fixture,
        id: ClientId,
        surface_local: Point<f64, Logical>,
    ) -> (
        WlSurface,
        impl Fn(Point<f64, Logical>) -> Point<f64, Logical>,
    ) {
        let viewer = f.niri_output(1);
        let steam = output_named(f, "steam");
        let window = map_window_on(f, id, &steam, 400, 300);
        let surface = f.client(id).state.windows.last().unwrap().surface.clone();
        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();

        let projection = view_projection(f);
        assert!((projection.scale() - 1.35).abs() < 1e-9, "{projection:?}");
        let centre = window_center_on(f.niri(), &steam, &window);
        let Some((_, HitType::Input { win_pos })) = f.niri().layout.window_under(&steam, centre)
        else {
            panic!("no input hit on the steam window");
        };
        let viewer_loc = geometry(f, &viewer).loc;
        let to_global =
            move |local: Point<f64, Logical>| projection.to_viewer(win_pos + local) + viewer_loc;

        f.niri_state().move_cursor(to_global(surface_local));
        // A real motion gives the surface pointer focus.
        input::pointer_motion(f, (0., 0.));
        assert_eq!(
            f.niri().pointer_contents.window.as_ref().map(|(w, _)| w),
            Some(&window)
        );
        (surface, to_global)
    }

    #[test]
    fn a_confine_region_is_checked_in_surface_pixels_under_projection_scale() {
        let mut f = set_up_with(Config::default(), (1920, 1080), &[("steam", 1280, 800)]);
        let id = f.add_client();
        let (surface, to_global) = view_window_under_pointer(&mut f, id, Point::from((150., 150.)));
        let _confined = f.client(id).confine_pointer(&surface, (0, 0, 200, 300));
        f.double_roundtrip(id);
        let start = pointer(&mut f);

        // 60 viewer pixels are 44.4 surface pixels: x = 194.4 stays inside the region.
        input::pointer_motion(&mut f, (60., 0.));
        let inside = pointer(&mut f);
        assert!(
            (inside - (start + Point::from((60., 0.)))).x.abs() < 1e-6,
            "the pointer was stopped at {inside:?} inside the confine region (from {start:?})"
        );
        assert!(
            (inside - to_global(Point::from((150. + 60. / 1.35, 150.))))
                .x
                .abs()
                < 1e-6
        );

        // Positive control: another 60 (x = 238.9) would leave it, and is prevented.
        input::pointer_motion(&mut f, (60., 0.));
        assert_eq!(pointer(&mut f), inside, "the confinement is not active");
    }

    #[test]
    fn a_lock_position_hint_lands_on_the_viewer_under_projection_scale() {
        let mut f = set_up_with(Config::default(), (1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let (surface, to_global) = view_window_under_pointer(&mut f, id, Point::from((100., 100.)));
        let locked = f.client(id).lock_pointer(&surface);
        f.double_roundtrip(id);
        let locked_at = pointer(&mut f);
        input::pointer_motion(&mut f, (30., 30.));
        assert_eq!(pointer(&mut f), locked_at, "the lock is not active");

        locked.set_cursor_position_hint(50., 60.);
        surface.commit();
        f.double_roundtrip(id);
        locked.destroy();
        f.double_roundtrip(id);

        let p = pointer(&mut f);
        let expected = to_global(Point::from((50., 60.)));
        let viewer_geo = geometry(&mut f, &viewer);
        assert!(
            viewer_geo.contains(p)
                && (p - expected).x.abs() < 1e-6
                && (p - expected).y.abs() < 1e-6,
            "unlocking warped the pointer to {p:?}, expected the hint at {expected:?} on the \
             viewer {viewer_geo:?}"
        );
    }
}
