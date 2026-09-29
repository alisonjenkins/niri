#![allow(clippy::unwrap_used, clippy::expect_used)]

use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::projection::{
    band_rect, column_layout, letterbox, overview_columns, scroll_to_show, ColumnSource,
    Projection, ProjectionError, ProjectionKind, BAND_FRACTION,
};

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

mod band_tests {
    use super::*;

    #[test]
    fn width_is_band_fraction_of_viewer_at_full_progress() {
        let band = band_rect(size(2000., 1000.), 1.);

        assert_eq!(band.size, size(2000. * BAND_FRACTION, 1000.));
        assert_eq!(band.loc, Point::from((2000. - 2000. * BAND_FRACTION, 0.)));
    }

    #[test]
    fn scales_linearly_with_progress() {
        let band = band_rect(size(2000., 1000.), 0.5);

        assert_eq!(band.size.w, 2000. * BAND_FRACTION * 0.5);
        assert_eq!(band.size.h, 1000.);
    }

    #[test]
    fn zero_width_at_progress_zero() {
        let band = band_rect(size(2000., 1000.), 0.);

        assert_eq!(band.size.w, 0.);
        assert_eq!(band.loc.x, 2000.);
    }

    #[test]
    fn progress_is_clamped_to_zero_one() {
        let over = band_rect(size(2000., 1000.), 3.);
        let under = band_rect(size(2000., 1000.), -3.);

        assert_eq!(over, band_rect(size(2000., 1000.), 1.));
        assert_eq!(under, band_rect(size(2000., 1000.), 0.));
    }

    #[test]
    fn nan_progress_is_treated_as_zero() {
        let band = band_rect(size(2000., 1000.), f64::NAN);

        assert_eq!(band, band_rect(size(2000., 1000.), 0.));
    }
}

mod column_layout_tests {
    use super::*;

    fn source(name: &str, w: f64, h: f64, count: usize, active: usize) -> ColumnSource {
        ColumnSource {
            name: name.to_string(),
            output_size: size(w, h),
            workspace_count: count,
            active_workspace: active,
        }
    }

    #[test]
    fn groups_are_in_input_order() {
        let band = rect(0., 0., 300., 2000.);
        let sources = [
            source("c", 1920., 1080., 1, 0),
            source("a", 1920., 1080., 1, 0),
            source("b", 1920., 1080., 1, 0),
        ];

        let layout = column_layout(band, &sources, 0.);

        let names: Vec<&str> = layout.groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["c", "a", "b"]);
    }

    #[test]
    fn tiles_are_equal_size_for_1_3_and_12_sources_of_the_same_aspect() {
        let band = rect(0., 0., 300., 20000.);

        let mut tile_sizes = Vec::new();
        for count in [1, 3, 12] {
            let sources: Vec<ColumnSource> = (0..count)
                .map(|i| source(&format!("s{i}"), 1920., 1080., 1, 0))
                .collect();
            let layout = column_layout(band, &sources, 0.);
            let sizes: Vec<Size<f64, Logical>> = layout
                .groups
                .iter()
                .flat_map(|g| g.tiles.iter().map(|t| t.region.size))
                .collect();
            tile_sizes.push(sizes);
        }

        for sizes in &tile_sizes {
            for size in sizes {
                assert_eq!(*size, tile_sizes[0][0]);
            }
        }
    }

    #[test]
    fn tiles_share_width_but_not_height_across_different_aspects() {
        let band = rect(0., 0., 300., 2000.);
        let sources = [
            source("wide", 1920., 1080., 1, 0),
            source("tall", 1080., 1920., 1, 0),
        ];

        let layout = column_layout(band, &sources, 0.);

        let wide_tile = layout.groups[0].tiles[0].region;
        let tall_tile = layout.groups[1].tiles[0].region;
        assert_eq!(wide_tile.size.w, tall_tile.size.w);
        assert_ne!(wide_tile.size.h, tall_tile.size.h);
    }

    #[test]
    fn scroll_clamps_to_the_content_range() {
        let band = rect(0., 0., 300., 200.);
        let sources = [source("s", 1920., 1080., 5, 0)];

        let below_zero = column_layout(band, &sources, -100.);
        let above_max = column_layout(band, &sources, 1_000_000.);
        let at_max = column_layout(band, &sources, above_max.content_h - band.size.h);

        assert_eq!(below_zero.scroll, 0.);
        assert_eq!(above_max.scroll, above_max.content_h - band.size.h);
        assert_eq!(at_max.scroll, above_max.scroll);
    }

    #[test]
    fn nan_scroll_is_zero() {
        let band = rect(0., 0., 300., 200.);
        let sources = [source("s", 1920., 1080., 5, 0)];

        let layout = column_layout(band, &sources, f64::NAN);

        assert_eq!(layout.scroll, 0.);
    }

    #[test]
    fn short_content_is_centred_and_unscrolled() {
        let band = rect(0., 0., 300., 20000.);
        let sources = [source("s", 1920., 1080., 1, 0)];

        let layout = column_layout(band, &sources, 500.);

        assert_eq!(layout.scroll, 0.);
        assert!(layout.content_h < band.size.h);
        let expected_top = (band.size.h - layout.content_h) / 2.;
        assert_eq!(layout.groups[0].label_rect.loc.y, expected_top);
    }

    fn nan_size_source() -> ColumnSource {
        let mut src = source("nan-size", 1920., 1080., 1, 0);
        // `Size::new` asserts on NaN, so build a valid size and corrupt it afterwards.
        src.output_size.w = f64::NAN;
        src
    }

    #[test]
    fn sources_with_bad_size_or_no_workspaces_are_skipped() {
        let band = rect(0., 0., 300., 2000.);
        let sources = [
            source("good", 1920., 1080., 1, 0),
            source("zero-size", 0., 1080., 1, 0),
            nan_size_source(),
            source("no-workspaces", 1920., 1080., 0, 0),
            source("also-good", 1920., 1080., 1, 0),
        ];

        let layout = column_layout(band, &sources, 0.);

        assert_eq!(layout.skipped, vec![1, 2, 3]);
        let names: Vec<&str> = layout.groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["good", "also-good"]);
    }

    #[test]
    fn zero_width_band_is_an_empty_layout() {
        let band = rect(0., 0., 0., 2000.);
        let sources = [source("s", 1920., 1080., 1, 0)];

        let layout = column_layout(band, &sources, 0.);

        assert!(layout.groups.is_empty());
        assert_eq!(layout.content_h, 0.);
    }

    #[test]
    fn active_workspace_is_marked() {
        let band = rect(0., 0., 300., 2000.);
        let sources = [source("s", 1920., 1080., 3, 1)];

        let layout = column_layout(band, &sources, 0.);

        let active: Vec<bool> = layout.groups[0].tiles.iter().map(|t| t.active).collect();
        assert_eq!(active, vec![false, true, false]);
    }
}

mod scroll_to_show_tests {
    use super::*;

    fn source(name: &str, w: f64, h: f64, count: usize, active: usize) -> ColumnSource {
        ColumnSource {
            name: name.to_string(),
            output_size: size(w, h),
            workspace_count: count,
            active_workspace: active,
        }
    }

    #[test]
    fn tile_already_in_view_keeps_the_current_scroll() {
        let band = rect(0., 0., 300., 200.);
        let sources = [source("s", 1920., 1080., 5, 0)];

        let scroll = scroll_to_show(band, &sources, 0., (0, 0));

        assert_eq!(scroll, 0.);
    }

    #[test]
    fn tile_below_the_view_scrolls_down_to_its_bottom() {
        let band = rect(0., 0., 300., 200.);
        let sources = [source("s", 1920., 1080., 5, 0)];
        let unscrolled = column_layout(band, &sources, 0.);
        let last = unscrolled.groups[0].tiles.last().unwrap();
        let expected = (last.region.loc.y + last.region.size.h - band.size.h)
            .clamp(0., (unscrolled.content_h - band.size.h).max(0.));

        let scroll = scroll_to_show(band, &sources, 0., (0, 4));

        assert_eq!(scroll, expected);
        assert!(scroll > 0.);
    }

    #[test]
    fn tile_above_the_view_scrolls_up_to_its_top() {
        let band = rect(0., 0., 300., 200.);
        let sources = [source("s", 1920., 1080., 5, 0)];
        let unscrolled = column_layout(band, &sources, 0.);
        let max_scroll = (unscrolled.content_h - band.size.h).max(0.);
        let first_tile_top = unscrolled.groups[0].tiles[0].region.loc.y;

        let scrolled_down = scroll_to_show(band, &sources, 0., (0, 4));
        let scroll_back_up = scroll_to_show(band, &sources, scrolled_down, (0, 0));

        assert_eq!(scroll_back_up, first_tile_top);
        assert!(scrolled_down > 0. && scrolled_down <= max_scroll);
    }
}

mod fixture_tests {
    use smithay::utils::{Logical, Point};

    use crate::niri::LockState;
    use crate::projection::{ProjectionKind, ViewOrigin, Viewing};
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
            origin: ViewOrigin::Command,
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
            origin: ViewOrigin::Command,
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
            origin: ViewOrigin::Command,
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
            origin: ViewOrigin::Command,
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
            origin: ViewOrigin::Command,
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
            origin: ViewOrigin::Command,
        });
        // As start_viewing() does; a viewer left active would end view mode on the next refresh.
        let steam = f
            .niri()
            .layout
            .outputs()
            .find(|o| o.name() == "steam")
            .cloned();
        f.niri().layout.focus_output(&steam.unwrap());
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
    use crate::projection::{Projection, ProjectionKind, ViewOrigin, Viewing};
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
            origin: ViewOrigin::Command,
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
    use crate::projection::{ProjectionKind, ViewOrigin, Viewing};
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

        let reply = f.niri().stop_viewing("test");

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

        assert_eq!(f.niri().stop_viewing("test"), ViewOutputState::NotViewing);
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
                origin: ViewOrigin::Command,
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
                origin: ViewOrigin::Overview,
            })
        );
        assert_eq!(active_output_name(&mut f), "steam");
    }

    #[test]
    fn clicking_a_source_workspace_on_another_monitor_moves_view_mode_there() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800)]);
        f.add_output(2, (1920, 1080));
        let first = f.niri_output(1);
        let second = f.niri_output(2);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        map_window_on(&mut f, id, &steam, 400, 300);
        f.niri().layout.focus_output(&first);
        start(&mut f, "steam");
        assert_eq!(viewing(&mut f).unwrap().viewer, first.name());
        open_overview(&mut f);
        let (ws_id, _) = workspace_on(&mut f, &steam, 1);

        f.niri().activate_overview_workspace(&steam, ws_id, &second);
        f.niri_complete_animations();

        assert_eq!(
            viewing(&mut f),
            Some(Viewing {
                viewer: second.name(),
                source: "steam".to_string(),
                origin: ViewOrigin::Overview,
            })
        );
    }

    #[test]
    fn a_virtual_input_output_keeps_the_current_viewer() {
        let mut f = set_up((1920, 1080), &[("steam", 1280, 800), ("other", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let other = output_named(&mut f, "other");
        map_window_on(&mut f, id, &steam, 400, 300);
        f.niri().layout.focus_output(&viewer);
        start(&mut f, "steam");
        open_overview(&mut f);
        let (ws_id, _) = workspace_on(&mut f, &steam, 1);

        f.niri().activate_overview_workspace(&steam, ws_id, &other);
        f.niri_complete_animations();

        assert_eq!(viewing(&mut f).unwrap().viewer, viewer.name());
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
    use niri_config::Action;
    use smithay::backend::renderer::element::Element as _;
    use smithay::backend::renderer::gles::GlesRenderer;
    use smithay::output::Output;
    use smithay::utils::{Logical, Physical, Point, Rectangle, Scale, Size};

    use super::overview_tests::map_window_on;
    use crate::niri::OutputRenderElements;
    use crate::projection::{Projection, ProjectionKind};
    use crate::render_helpers::{RenderCtx, RenderTarget};
    use crate::tests::fixture::Fixture;
    use crate::tests::input;

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
    fn egl_overview_renders_each_source_inside_its_region() {
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
    fn egl_overview_labels_each_column_with_its_source_name() {
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
    fn egl_rendering_a_source_never_includes_projections() {
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
    fn egl_a_viewer_redraw_refreshes_the_sources_it_projects() {
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
    fn egl_overview_frame_has_unique_element_ids_with_a_notification_showing() {
        let mut f = set_up(&[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        f.niri().config_error_notification.show();
        f.niri_complete_animations();

        let elements = render(&mut f, &viewer);

        assert_unique_ids(&elements);
    }

    /// A viewer point inside steam's column where one of the viewer's own windows, scrolled
    /// off its workspace to the right, is also laid out.
    fn overflow_point(f: &mut Fixture, viewer: &Output) -> Point<f64, Logical> {
        let region = overview_projections(f)
            .into_iter()
            .find(|p| p.source == "steam")
            .unwrap()
            .region;
        let niri = f.niri();
        (0..20)
            .flat_map(|i| (0..20).map(move |j| (i, j)))
            .map(|(i, j)| {
                region.loc
                    + Point::from((
                        region.size.w * (f64::from(i) + 0.5) / 20.,
                        region.size.h * (f64::from(j) + 0.5) / 20.,
                    ))
            })
            .find(|p| niri.layout.window_under(viewer, *p).is_some())
            .unwrap_or_else(|| panic!("no viewer window overflows into the column {region:?}"))
    }

    #[test]
    fn egl_a_viewer_window_overflowing_into_a_column_is_drawn_under_it() {
        let mut f = Fixture::new();
        f.niri_state().backend.headless().add_renderer().unwrap();
        f.add_output(1, (5120, 1440));
        let viewer = f.niri_output(1);
        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();
        let id = f.add_client();
        for _ in 0..6 {
            map_window_on(&mut f, id, &viewer, 1600, 600);
        }
        f.niri_state().do_action(Action::FocusColumnFirst, false);
        f.niri().layout.toggle_overview();
        f.niri_complete_animations();
        f.niri_state().refresh_and_flush_clients();
        let point = overflow_point(&mut f, &viewer);

        // Elements are front to back, so the first one covering the point is what is seen.
        let scale = Scale::from(viewer.current_scale().fractional_scale());
        let pixel = Rectangle::new(point.to_physical_precise_round(scale), Size::from((1, 1)));
        let elements = render(&mut f, &viewer);
        let top = elements
            .iter()
            .find(|e| e.geometry(scale).contains_rect(pixel))
            .unwrap();
        assert!(
            !matches!(top, OutputRenderElements::Monitor(_)),
            "the viewer's own window is drawn over steam's column at {point:?}"
        );

        // The click lands where the picture says: steam's column.
        let target = point - f.niri().seat.get_pointer().unwrap().current_location();
        input::pointer_motion(&mut f, target);
        assert_eq!(
            f.niri().pointer_contents.output.as_ref().map(|o| o.name()),
            Some("steam".to_string())
        );
        input::pointer_button(&mut f, input::BTN_LEFT, true);
        input::pointer_button(&mut f, input::BTN_LEFT, false);
        f.niri_complete_animations();
        assert_eq!(
            f.niri().layout.active_output().map(|o| o.name()),
            Some("steam".to_string())
        );
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
    fn egl_view_mode_draws_the_source_letterboxed_over_a_black_backdrop() {
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
    fn egl_view_mode_draws_the_viewing_label_on_the_viewer() {
        let (mut f, viewer) = set_up();

        let elements = render(&mut f, &viewer);

        assert!(elements
            .iter()
            .any(|e| matches!(e, OutputRenderElements::Texture(_))));
    }

    #[test]
    fn egl_view_mode_frame_has_unique_element_ids_with_a_notification_showing() {
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
    use crate::projection::{Projection, ProjectionKind, ViewOrigin};
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

    /// Moves the pointer to `target` with relative motion, the way a mouse does.
    fn move_pointer_to(f: &mut Fixture, target: Point<f64, Logical>) {
        let delta = target - pointer(f);
        input::pointer_motion(f, delta);
    }

    /// What the event loop runs after dispatching each batch of events in a session.
    fn refresh(f: &mut Fixture) {
        f.niri_state().refresh_and_flush_clients();
    }

    fn click(f: &mut Fixture) {
        input::pointer_button(f, input::BTN_LEFT, true);
        refresh(f);
        input::pointer_button(f, input::BTN_LEFT, false);
        refresh(f);
        f.niri_complete_animations();
        refresh(f);
    }

    fn toggle_overview(f: &mut Fixture) {
        f.niri_state().do_action(Action::ToggleOverview, false);
        refresh(f);
        f.niri_complete_animations();
        refresh(f);
    }

    /// Where workspace `idx` of `output` is drawn on the viewer while the overview is open:
    /// near its top edge, which stays on screen for the workspace below the active one.
    fn overview_workspace_point(
        f: &mut Fixture,
        viewer: &Output,
        output: &Output,
        idx: usize,
    ) -> Point<f64, Logical> {
        let viewer_geo = geometry(f, viewer);
        let niri = f.niri();
        let ws = niri
            .layout
            .monitor_for_output(output)
            .unwrap()
            .workspaces_render_geo()
            .nth(idx)
            .unwrap();
        let local = Point::from((center(ws).x, ws.loc.y + 40.));
        let on_viewer = if output == viewer {
            local
        } else {
            niri.projection_state
                .projections
                .iter()
                .find(|p| p.kind == ProjectionKind::Overview && p.source == output.name())
                .unwrap()
                .to_viewer(local)
        };
        on_viewer + viewer_geo.loc
    }

    /// The desk set-up from the bug report: a window on the viewer and one on steam, the
    /// viewer active, then steam's second workspace clicked in the overview.
    fn view_steam_from_the_overview() -> (Fixture, Output, Window) {
        view_steam_from_the_overview_with(warp_config())
    }

    fn view_steam_from_the_overview_with(config: Config) -> (Fixture, Output, Window) {
        let mut f = set_up_with(config, (5120, 1440), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        map_window_on(&mut f, id, &steam, 400, 300);
        let viewer_window = map_window_on(&mut f, id, &viewer, 400, 300);
        f.niri().layout.focus_output(&viewer);
        let viewer_centre = center(geometry(&mut f, &viewer));
        f.niri_state().move_cursor(viewer_centre);

        toggle_overview(&mut f);
        let target = overview_workspace_point(&mut f, &viewer, &steam, 1);
        move_pointer_to(&mut f, target);
        click(&mut f);

        let viewing = f.niri().projection_state.viewing.clone().unwrap();
        assert_eq!(viewing.viewer, viewer.name());
        assert_eq!(viewing.source, "steam");
        assert!(!f.niri().layout.is_overview_open());
        assert_eq!(f.niri().layout.active_output(), Some(&steam));
        (f, viewer, viewer_window)
    }

    fn assert_back_on_the_viewer(f: &mut Fixture, viewer: &Output, ws_idx: usize) {
        let niri = f.niri();
        assert_eq!(niri.projection_state.viewing, None);
        assert!(!niri.layout.is_overview_open());
        assert_eq!(niri.layout.active_output(), Some(viewer));
        assert_eq!(
            niri.layout
                .monitor_for_output(viewer)
                .unwrap()
                .active_workspace_idx(),
            ws_idx
        );
        assert!(
            niri.projection_state
                .projections
                .iter()
                .all(|p| p.kind != ProjectionKind::View),
            "the view projection still covers the viewer: {:?}",
            niri.projection_state.projections
        );
    }

    #[test]
    fn clicking_an_empty_viewer_workspace_after_viewing_from_the_overview_returns() {
        let (mut f, viewer, _window) = view_steam_from_the_overview();

        toggle_overview(&mut f);
        let target = overview_workspace_point(&mut f, &viewer, &viewer, 1);
        move_pointer_to(&mut f, target);
        click(&mut f);

        assert_back_on_the_viewer(&mut f, &viewer, 1);
    }

    #[test]
    fn clicking_a_viewer_window_after_viewing_from_the_overview_returns() {
        let (mut f, viewer, window) = view_steam_from_the_overview();

        toggle_overview(&mut f);
        let target = window_center_on(f.niri(), &viewer, &window) + geometry(&mut f, &viewer).loc;
        move_pointer_to(&mut f, target);
        assert!(
            f.niri().window_under_cursor().is_some(),
            "the click should land on the viewer's window"
        );
        click(&mut f);

        assert_back_on_the_viewer(&mut f, &viewer, 0);
    }

    /// The desk's overview and monitor binds; steam lies right of the viewer.
    fn keyboard_config() -> Config {
        let mut config = Config::parse_mem(
            "binds {
                Mod+O { toggle-overview; }
                Mod+Shift+Left { focus-monitor-left; }
            }",
        )
        .unwrap();
        config.input.warp_mouse_to_focus = warp_config().input.warp_mouse_to_focus;
        config
    }

    /// Presses `keys` in order and releases them in reverse.
    fn press(f: &mut Fixture, keys: &[&str]) {
        for name in keys {
            input::key(f, name, true);
            refresh(f);
        }
        for name in keys.iter().rev() {
            input::key(f, name, false);
            refresh(f);
        }
        f.niri_complete_animations();
        refresh(f);
    }

    #[test]
    fn picking_the_viewer_with_the_keyboard_in_the_overview_returns() {
        let (mut f, viewer, _window) = view_steam_from_the_overview_with(keyboard_config());

        press(&mut f, &["LWIN", "AD09"]);
        assert!(f.niri().layout.is_overview_open());
        press(&mut f, &["LWIN", "LFSH", "LEFT"]);
        press(&mut f, &["RTRN"]);

        assert_back_on_the_viewer(&mut f, &viewer, 0);
    }

    #[test]
    fn focusing_the_viewer_in_view_mode_returns_to_it() {
        let (mut f, viewer, _window) = view_steam_from_the_overview_with(keyboard_config());

        press(&mut f, &["LWIN", "LFSH", "LEFT"]);

        assert_back_on_the_viewer(&mut f, &viewer, 0);
    }

    /// A focused app window on steam, viewed either from the overview (by clicking the window
    /// in steam's column) or by command. Returns the app's surface to read its key events.
    fn app_in_view(origin: ViewOrigin) -> (Fixture, Output, ClientId, WlSurface) {
        let mut f = set_up_with(keyboard_config(), (5120, 1440), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        f.niri().layout.focus_output(&steam);
        let app = f.client(id).create_window();
        let surface = app.surface.clone();
        app.commit();
        f.roundtrip(id);
        let app = f.client(id).window(&surface);
        app.attach_new_buffer();
        app.set_size(400, 300);
        app.ack_last_and_commit();
        f.double_roundtrip(id);
        let window = f
            .niri()
            .layout
            .windows_for_output(&steam)
            .next()
            .unwrap()
            .window
            .clone();
        map_window_on(&mut f, id, &viewer, 400, 300);
        f.niri().layout.focus_output(&viewer);
        let viewer_centre = center(geometry(&mut f, &viewer));
        f.niri_state().move_cursor(viewer_centre);

        match origin {
            ViewOrigin::Overview => {
                toggle_overview(&mut f);
                let projection = overview_projection(&mut f, "steam");
                let target = projection.to_viewer(window_center_on(f.niri(), &steam, &window))
                    + geometry(&mut f, &viewer).loc;
                move_pointer_to(&mut f, target);
                click(&mut f);
            }
            ViewOrigin::Command => {
                f.niri_state().view_output(Some("steam")).unwrap();
                refresh(&mut f);
            }
        }
        f.double_roundtrip(id);

        let viewing = f.niri().projection_state.viewing.clone().unwrap();
        assert_eq!(viewing.origin, origin);
        assert_eq!(f.niri().layout.active_output(), Some(&steam));
        let _ = f.client(id).state.recent_keyboard_events(&surface);
        (f, viewer, id, surface)
    }

    fn key_events(f: &mut Fixture, id: ClientId, surface: &WlSurface) -> Vec<String> {
        f.double_roundtrip(id);
        f.client(id)
            .state
            .recent_keyboard_events(surface)
            .map(|event| event.to_string())
            .filter(|event| event.starts_with("key "))
            .collect()
    }

    const KEY_ESC: &str = "1";

    #[test]
    fn escape_leaves_a_view_entered_from_the_overview_without_reaching_the_app() {
        let (mut f, viewer, id, surface) = app_in_view(ViewOrigin::Overview);
        assert_eq!(
            f.niri().view_output_label.text_on(&viewer.name()),
            Some("Viewing: steam — Esc to return")
        );

        press(&mut f, &["ESC"]);

        assert_back_on_the_viewer(&mut f, &viewer, 0);
        assert_eq!(key_events(&mut f, id, &surface), Vec::<String>::new());
    }

    #[test]
    fn escape_goes_to_the_app_in_a_view_entered_by_command() {
        let (mut f, viewer, id, surface) = app_in_view(ViewOrigin::Command);
        assert_eq!(
            f.niri().view_output_label.text_on(&viewer.name()),
            Some("Viewing: steam")
        );

        press(&mut f, &["ESC"]);

        assert!(f.niri().projection_state.viewing.is_some());
        assert_eq!(
            key_events(&mut f, id, &surface),
            [
                format!("key pressed: {KEY_ESC}"),
                format!("key released: {KEY_ESC}")
            ]
        );
    }

    #[test]
    fn escape_goes_to_a_shortcut_inhibiting_app_in_a_view_from_the_overview() {
        let (mut f, _viewer, id, surface) = app_in_view(ViewOrigin::Overview);
        let _inhibitor = f.client(id).state.inhibit_shortcuts(&surface);
        f.roundtrip(id);

        press(&mut f, &["ESC"]);

        assert!(f.niri().projection_state.viewing.is_some());
        assert_eq!(
            key_events(&mut f, id, &surface),
            [
                format!("key pressed: {KEY_ESC}"),
                format!("key released: {KEY_ESC}")
            ]
        );
    }

    #[test]
    fn escape_in_the_overview_over_a_view_from_the_overview_only_closes_the_overview() {
        let (mut f, _viewer, id, surface) = app_in_view(ViewOrigin::Overview);
        let steam = output_named(&mut f, "steam");
        press(&mut f, &["LWIN", "AD09"]);
        assert!(f.niri().layout.is_overview_open());

        press(&mut f, &["ESC"]);

        assert!(!f.niri().layout.is_overview_open());
        assert!(f.niri().projection_state.viewing.is_some());
        assert_eq!(f.niri().layout.active_output(), Some(&steam));
        let events = key_events(&mut f, id, &surface);
        assert!(
            !events.iter().any(|e| e.ends_with(&format!(": {KEY_ESC}"))),
            "the overview's Escape reached the app: {events:?}"
        );
    }

    #[test]
    fn modified_escape_goes_to_the_app_in_a_view_from_the_overview() {
        let (mut f, _viewer, id, surface) = app_in_view(ViewOrigin::Overview);

        press(&mut f, &["LFSH", "ESC"]);

        assert!(f.niri().projection_state.viewing.is_some());
        let events = key_events(&mut f, id, &surface);
        assert!(
            events.contains(&format!("key pressed: {KEY_ESC}")),
            "Shift+Escape did not reach the app: {events:?}"
        );
    }

    fn active_view_pos(f: &mut Fixture, output: &Output) -> f64 {
        f.niri()
            .layout
            .monitor_for_output(output)
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
        let view_pos_before = active_view_pos(&mut f, &steam);

        input::super_key(&mut f, true);
        input::pointer_button(&mut f, input::BTN_MIDDLE, true);
        // 135 viewer pixels are 100 source pixels at scale 1.35.
        for _ in 0..3 {
            input::pointer_motion(&mut f, (45., 0.));
        }
        let scrolled = active_view_pos(&mut f, &steam) - view_pos_before;
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

    #[test]
    fn right_drag_in_a_shrunk_source_column_scrolls_the_source_at_projection_scale() {
        // A 4K source next to a 1080p viewer's strip only fits shrunk.
        let mut f = set_up_with(Config::default(), (1920, 1080), &[("steam", 3840, 2160)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        for _ in 0..3 {
            map_window_on(&mut f, id, &steam, 3000, 1000);
        }
        f.niri().layout.focus_output(&viewer);
        open_overview(&mut f);
        let projection = overview_projection(&mut f, "steam");
        let scale = projection.scale();
        assert!(scale < 1., "the steam column is not shrunk: {projection:?}");
        let zoom = f.niri().layout.overview_zoom();
        let viewer_geo = geometry(&mut f, &viewer);
        f.niri_state()
            .move_cursor(center(projection.region) + viewer_geo.loc);
        let view_pos_before = active_view_pos(&mut f, &steam);

        input::pointer_button(&mut f, input::BTN_RIGHT, true);
        for _ in 0..3 {
            input::pointer_motion(&mut f, (10., 0.));
        }
        let scrolled = active_view_pos(&mut f, &steam) - view_pos_before;
        input::pointer_button(&mut f, input::BTN_RIGHT, false);

        let expected = -30. / scale / zoom;
        assert!(
            (scrolled - expected).abs() < 1e-6,
            "a 30 px drag at projection scale {scale} and zoom {zoom} scrolled steam by \
             {scrolled} px, expected {expected}"
        );
    }

    /// Two 1920x1080 monitors side by side with no virtual outputs, and three wide windows
    /// on the left one so its view can scroll.
    fn set_up_unprojected() -> (Fixture, Output) {
        let mut f = set_up_with(Config::default(), (1920, 1080), &[]);
        f.add_output(2, (1920, 1080));
        let left = f.niri_output(1);
        let id = f.add_client();
        for _ in 0..3 {
            map_window_on(&mut f, id, &left, 1000, 300);
        }
        f.niri().layout.focus_output(&left);
        f.niri_complete_animations();
        assert!(f.niri().projection_state.projections.is_empty());
        (f, left)
    }

    #[test]
    fn unprojected_right_drag_in_the_overview_wraps_on_its_output_at_scale_one() {
        let (mut f, left) = set_up_unprojected();
        open_overview(&mut f);
        assert!(f.niri().projection_state.projections.is_empty());
        let zoom = f.niri().layout.overview_zoom();
        f.niri_state().move_cursor(Point::from((1900., 540.)));
        let view_pos_before = active_view_pos(&mut f, &left);

        input::pointer_button(&mut f, input::BTN_RIGHT, true);
        for _ in 0..3 {
            input::pointer_motion(&mut f, (20., 0.));
        }
        let scrolled = active_view_pos(&mut f, &left) - view_pos_before;
        let p = pointer(&mut f);
        input::pointer_button(&mut f, input::BTN_RIGHT, false);

        assert!(
            (p - Point::from((40., 540.))).x.abs() < 1e-6,
            "the pointer should have wrapped to the left monitor's left edge, is at {p:?}"
        );
        let expected = -60. / zoom;
        assert!(
            (scrolled - expected).abs() < 1e-6,
            "a 60 px drag at zoom {zoom} scrolled by {scrolled} px, expected {expected}"
        );
    }

    #[test]
    fn unprojected_mod_middle_drag_wraps_on_its_output_at_scale_one() {
        let (mut f, left) = set_up_unprojected();
        f.niri_state().move_cursor(Point::from((1900., 540.)));
        let view_pos_before = active_view_pos(&mut f, &left);

        input::super_key(&mut f, true);
        input::pointer_button(&mut f, input::BTN_MIDDLE, true);
        // Past the 8 px threshold leftwards, so the drag becomes horizontal before the wrap.
        input::pointer_motion(&mut f, (-10., 0.));
        for _ in 0..3 {
            input::pointer_motion(&mut f, (20., 0.));
        }
        let scrolled = active_view_pos(&mut f, &left) - view_pos_before;
        let p = pointer(&mut f);
        input::pointer_button(&mut f, input::BTN_MIDDLE, false);
        input::super_key(&mut f, false);

        assert!(
            (p - Point::from((30., 540.))).x.abs() < 1e-6,
            "the pointer should have wrapped to the left monitor's left edge, is at {p:?}"
        );
        assert!(
            (scrolled + 50.).abs() < 1e-6,
            "a 50 px drag scrolled by {scrolled} px, expected -50"
        );
    }

    #[test]
    fn a_lock_position_hint_in_a_cropped_part_of_a_surface_stays_in_the_projection() {
        let mut f = set_up_with(Config::default(), (1920, 1080), &[("steam", 1280, 800)]);
        let viewer = f.niri_output(1);
        let id = f.add_client();
        let steam = output_named(&mut f, "steam");
        let window = map_window_on(&mut f, id, &steam, 1000, 300);
        let surface = f.client(id).state.windows.last().unwrap().surface.clone();
        // A second wide window scrolls the view so the first hangs off steam's left edge.
        map_window_on(&mut f, id, &steam, 1000, 300);
        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();
        let projection = view_projection(&mut f);

        let centre = window_center_on(f.niri(), &steam, &window);
        let left = centre.x - 500.;
        assert!(
            left < 0. && left + 1000. > 40.,
            "the window {left}..{} is not cropped by steam's left edge",
            left + 1000.
        );
        let viewer_geo = geometry(&mut f, &viewer);
        // A visible part of the window.
        let visible = Point::from((20., centre.y));
        f.niri_state()
            .move_cursor(projection.to_viewer(visible) + viewer_geo.loc);
        input::pointer_motion(&mut f, (0., 0.));
        assert_eq!(
            f.niri().pointer_contents.window.as_ref().map(|(w, _)| w),
            Some(&window)
        );

        let locked = f.client(id).lock_pointer(&surface);
        f.double_roundtrip(id);
        // Surface-local x = 10 lies left of steam's edge, in the part the projection crops.
        locked.set_cursor_position_hint(10., 60.);
        surface.commit();
        f.double_roundtrip(id);
        locked.destroy();
        f.double_roundtrip(id);

        let p = pointer(&mut f);
        let region = Rectangle::new(
            projection.region.loc + viewer_geo.loc,
            projection.region.size,
        );
        assert!(
            region.contains(p),
            "unlocking warped the pointer to {p:?}, outside the projection {region:?}"
        );
        assert!(
            (p.x - region.loc.x).abs() < 1e-6,
            "the cropped hint should clamp to the projection's left edge {}, got {p:?}",
            region.loc.x
        );
        assert_eq!(
            f.niri().pointer_contents.window.as_ref().map(|(w, _)| w),
            Some(&window),
            "the pointer should still be over the window after unlocking"
        );
    }
}

/// The screenshot UI while a virtual output is projected: it is drawn on each physical
/// output, so a drag on the viewer must select on the viewer, never on the source it shows.
mod screenshot_tests {
    use niri_config::Action;
    use smithay::output::Output;
    use smithay::utils::{Logical, Physical, Point, Rectangle};

    use super::overview_tests::{map_window_on, output_named};
    use crate::projection::ProjectionKind;
    use crate::tests::fixture::Fixture;
    use crate::tests::input;

    /// A viewer with a window of its own and a 1280x800 `steam` with a window on it.
    pub(super) fn set_up(viewer_size: (u16, u16)) -> (Fixture, Output) {
        let mut f = Fixture::new();
        f.niri_state().backend.headless().add_renderer().unwrap();
        f.add_output(1, viewer_size);
        let state = f.niri_state();
        state
            .backend
            .headless()
            .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
            .unwrap();
        let viewer = f.niri_output(1);
        let steam = output_named(&mut f, "steam");
        let id = f.add_client();
        map_window_on(&mut f, id, &steam, 300, 200);
        map_window_on(&mut f, id, &viewer, 300, 200);
        f.niri().layout.focus_output(&viewer);
        (f, viewer)
    }

    pub(super) fn viewer_loc(f: &mut Fixture, viewer: &Output) -> Point<f64, Logical> {
        f.niri()
            .global_space
            .output_geometry(viewer)
            .unwrap()
            .loc
            .to_f64()
    }

    fn region_of(f: &mut Fixture, kind: ProjectionKind) -> Rectangle<f64, Logical> {
        f.niri()
            .projection_state
            .projections
            .iter()
            .find(|p| p.kind == kind && p.source == "steam")
            .unwrap()
            .region
    }

    fn open_screenshot_ui(f: &mut Fixture) {
        f.niri_state()
            .do_action(Action::Screenshot(true, None), false);
        assert!(f.niri().screenshot_ui.is_open());
    }

    fn selection(f: &mut Fixture) -> (String, Rectangle<i32, Physical>) {
        let (output, rect) = f.niri().screenshot_ui.selection().unwrap();
        (output.name(), rect)
    }

    /// Presses at `start` (viewer-local), drags by `delta` and releases; returns the
    /// selection the drag is expected to make on a scale-1 viewer.
    fn drag(
        f: &mut Fixture,
        viewer: &Output,
        start: Point<f64, Logical>,
        delta: (f64, f64),
    ) -> Rectangle<i32, Physical> {
        assert_eq!(viewer.current_scale().fractional_scale(), 1.);
        let loc = viewer_loc(f, viewer);
        f.niri_state().move_cursor(start + loc);
        input::pointer_button(f, input::BTN_LEFT, true);
        input::pointer_motion(f, delta);
        input::pointer_button(f, input::BTN_LEFT, false);
        let start = start.to_physical(1.).to_i32_round::<i32>();
        // Selections include both corner pixels.
        Rectangle::new(start, (delta.0 as i32 + 1, delta.1 as i32 + 1).into())
    }

    #[test]
    fn egl_dragging_in_the_screenshot_ui_in_view_mode_selects_on_the_viewer() {
        let (mut f, viewer) = set_up((1920, 1080));
        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();
        let region = region_of(&mut f, ProjectionKind::View);
        open_screenshot_ui(&mut f);

        let start = region.loc + Point::from((200., 100.));
        let expected = drag(&mut f, &viewer, start, (400., 300.));

        assert!(f.niri().screenshot_ui.is_open());
        assert_eq!(
            selection(&mut f),
            (viewer.name(), expected),
            "the drag should select on the viewer the screenshot UI is drawn on"
        );
    }

    #[test]
    fn egl_opening_the_screenshot_ui_in_view_mode_selects_on_the_viewer() {
        // Over the source's picture and over a letterbox bar.
        for x in [960., 10.] {
            let (mut f, viewer) = set_up((1920, 1080));
            f.niri().start_viewing("steam").unwrap();
            f.niri_complete_animations();
            let loc = viewer_loc(&mut f, &viewer);
            f.niri_state().move_cursor(loc + Point::from((x, 540.)));

            open_screenshot_ui(&mut f);

            assert_eq!(
                selection(&mut f),
                (
                    viewer.name(),
                    Rectangle::new((480, 270).into(), (960, 540).into())
                ),
                "opening with the pointer at x={x} on the viewer should default to the viewer"
            );
        }
    }

    #[test]
    fn egl_dragging_in_the_screenshot_ui_without_projections_selects_under_the_pointer() {
        let (mut f, viewer) = set_up((1920, 1080));
        assert!(f.niri().projection_state.projections.is_empty());
        open_screenshot_ui(&mut f);

        let expected = drag(&mut f, &viewer, Point::from((300., 200.)), (400., 300.));

        assert_eq!(selection(&mut f), (viewer.name(), expected));
    }

    #[test]
    fn egl_dragging_in_the_screenshot_ui_over_an_overview_column_selects_on_the_viewer() {
        let (mut f, viewer) = set_up((5120, 1440));
        f.niri().layout.toggle_overview();
        f.niri_complete_animations();
        assert!(f.niri().layout.is_overview_open());
        let region = region_of(&mut f, ProjectionKind::Overview);
        open_screenshot_ui(&mut f);

        let start = region.loc + region.size.downscale(2.).to_point();
        let expected = drag(&mut f, &viewer, start, (50., 40.));

        assert_eq!(
            selection(&mut f),
            (viewer.name(), expected),
            "the drag should select on the viewer, not in the projected column's source"
        );
    }

    #[test]
    fn egl_tapping_in_the_screenshot_ui_in_view_mode_selects_on_the_viewer() {
        let (mut f, viewer) = set_up((1920, 1080));
        input::add_device(&mut f);
        f.niri().config.borrow_mut().input.touch.map_to_output = Some(viewer.name());
        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();
        let region = region_of(&mut f, ProjectionKind::View);
        open_screenshot_ui(&mut f);

        let tap = region.loc + Point::from((200., 100.));
        input::touch_tap(&mut f, (tap.x / 1920., tap.y / 1080.));

        // A tap selects the default 32x32 square centred on it.
        let loc = tap.to_physical(1.).to_i32_round::<i32>() - Point::from((16, 16));
        assert_eq!(
            selection(&mut f),
            (viewer.name(), Rectangle::new(loc, (32, 32).into())),
            "the tap should select on the viewer the screenshot UI is drawn on"
        );
    }
}

/// The colour picker while a virtual output is viewed. It renders from inside its pointer grab,
/// where the pointer is locked, and must sample what the physical output shows.
mod pick_color_tests {
    use niri_ipc::PickedColor;
    use smithay::utils::{Logical, Point};

    use super::screenshot_tests::{set_up, viewer_loc};
    use crate::tests::fixture::Fixture;
    use crate::tests::input;

    fn pick_at(f: &mut Fixture, pos: Point<f64, Logical>) -> Option<PickedColor> {
        let (tx, rx) = async_channel::unbounded();
        f.niri_state().move_cursor(pos);
        f.niri_state().handle_pick_color(tx);
        input::pointer_button(f, input::BTN_LEFT, true);
        input::pointer_button(f, input::BTN_LEFT, false);
        rx.try_recv().expect("the pick should have answered")
    }

    #[test]
    fn egl_picking_over_the_viewed_picture_answers() {
        let (mut f, viewer) = set_up((1920, 1080));
        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();
        let loc = viewer_loc(&mut f, &viewer);

        assert!(pick_at(&mut f, loc + Point::from((960., 540.))).is_some());
    }

    #[test]
    fn egl_picking_a_letterbox_bar_in_view_mode_returns_its_black() {
        let (mut f, viewer) = set_up((1920, 1080));
        f.niri().start_viewing("steam").unwrap();
        f.niri_complete_animations();
        let loc = viewer_loc(&mut f, &viewer);

        let color = pick_at(&mut f, loc + Point::from((10., 540.)));

        assert_eq!(
            color.map(|c| c.rgb),
            Some([0., 0., 0.]),
            "the bar is drawn black on the viewer, so picking it should return black"
        );
    }

    #[test]
    fn egl_picking_without_projections_samples_the_output_under_the_pointer() {
        let (mut f, viewer) = set_up((1920, 1080));
        let loc = viewer_loc(&mut f, &viewer);

        assert!(pick_at(&mut f, loc + Point::from((10., 540.))).is_some());
    }
}

mod workspace_overview_render_tests {
    use smithay::backend::renderer::element::{Element as _, Id};
    use smithay::backend::renderer::gles::GlesRenderer;
    use smithay::output::Output;
    use smithay::utils::{Physical, Rectangle, Scale};

    use super::overview_tests::map_window_on;
    use crate::layout::monitor::MonitorRenderElement;
    use crate::render_helpers::{RenderCtx, RenderTarget};
    use crate::tests::fixture::Fixture;

    type Rendered = Vec<(Id, Rectangle<i32, Physical>)>;

    enum Draw {
        Workspace(usize),
        Workspaces,
        Shadows,
    }

    const OUTPUT_H: i32 = 1080;

    /// Windows on workspaces 0, 1 and 2 and the overview open on workspace 0: workspaces 0 and
    /// 1 are on the output, 2 and the empty last one are below it.
    fn set_up() -> (Fixture, Output) {
        let mut f = Fixture::new();
        f.niri_state().backend.headless().add_renderer().unwrap();
        f.add_output(1, (1920, 1080));
        let output = f.niri_output(1);
        let id = f.add_client();
        for _ in 0..3 {
            map_window_on(&mut f, id, &output, 300, 200);
            f.niri().layout.switch_workspace_down();
        }
        f.niri().layout.switch_workspace(0);
        f.niri().layout.toggle_overview();
        f.niri_complete_animations();
        (f, output)
    }

    fn render(f: &mut Fixture, output: &Output, draw: Draw) -> Rendered {
        f.niri().update_render_elements(None);
        let scale = Scale::from(output.current_scale().fractional_scale());
        let state = f.niri_state();
        let mon = state.niri.layout.monitor_for_output(output).unwrap();
        let mut rendered = Vec::new();
        state
            .backend
            .headless()
            .with_primary_renderer(|renderer| {
                let mut push = |elem: MonitorRenderElement<GlesRenderer>| {
                    rendered.push((elem.id().clone(), elem.geometry(scale)));
                };
                let ctx = RenderCtx {
                    renderer,
                    target: RenderTarget::Output,
                    xray: None,
                };
                match draw {
                    Draw::Workspace(idx) => {
                        mon.render_workspace_overview(idx, ctx, true, &mut push)
                    }
                    Draw::Workspaces => mon.render_workspaces(ctx, true, &mut push),
                    Draw::Shadows => mon.render_workspace_shadows(ctx.renderer, &mut push),
                }
            })
            .unwrap();
        rendered
    }

    /// The background element and physical geometry of each workspace, including offscreen ones.
    fn backgrounds(f: &mut Fixture, output: &Output) -> Vec<(Id, Rectangle<i32, Physical>)> {
        let scale = Scale::from(output.current_scale().fractional_scale());
        let mon = f.niri().layout.monitor_for_output(output).unwrap();
        mon.workspaces_with_render_geo_cull(false)
            .map(|(ws, geo)| {
                let id = ws.render_background().id().clone();
                (id, geo.to_physical_precise_round(scale))
            })
            .collect()
    }

    /// Renders each workspace on its own, checking and dropping its background.
    fn render_each(f: &mut Fixture, output: &Output) -> Vec<Rendered> {
        let backgrounds = backgrounds(f, output);
        backgrounds
            .iter()
            .enumerate()
            .map(|(idx, background)| {
                let mut rendered = render(f, output, Draw::Workspace(idx));
                let count = rendered.len();
                rendered.retain(|elem| elem != background);
                assert_eq!(
                    rendered.len(),
                    count - 1,
                    "workspace {idx} has one background"
                );
                rendered
            })
            .collect()
    }

    // Ids have interior mutability, so they cannot key a HashSet.
    #[track_caller]
    fn assert_same_elements(actual: &Rendered, expected: &Rendered) {
        assert_eq!(actual.len(), expected.len(), "{actual:?} != {expected:?}");
        for elem in actual {
            assert!(
                expected.contains(elem),
                "{elem:?} missing from {expected:?}"
            );
        }
        for elem in expected {
            assert!(actual.contains(elem), "{elem:?} not rendered in {actual:?}");
        }
    }

    #[test]
    fn egl_render_workspace_overview_matches_the_overview_per_workspace() {
        let (mut f, output) = set_up();

        let mut whole = render(&mut f, &output, Draw::Workspaces);
        whole.extend(render(&mut f, &output, Draw::Shadows));
        let each = render_each(&mut f, &output);

        // Only the on-screen workspaces are in the overview's own render.
        let on_screen: Rendered = each[..2].concat();
        assert!(!each[0].is_empty() && !each[1].is_empty());
        assert_same_elements(&on_screen, &whole);
    }

    #[test]
    fn egl_render_workspace_overview_matches_an_unculled_overview() {
        let (mut f, output) = set_up();
        f.niri()
            .layout
            .set_overview_offscreen_reachable(&output, true);

        let mut whole = render(&mut f, &output, Draw::Workspaces);
        whole.extend(render(&mut f, &output, Draw::Shadows));
        let all: Rendered = render_each(&mut f, &output).concat();

        assert_same_elements(&all, &whole);
    }

    #[test]
    fn egl_render_workspace_overview_draws_an_offscreen_workspace_where_it_is() {
        let (mut f, output) = set_up();

        let whole = render(&mut f, &output, Draw::Workspaces);
        let offscreen = render(&mut f, &output, Draw::Workspace(2));

        assert!(!offscreen.is_empty());
        for (id, geo) in &offscreen {
            assert!(
                whole.iter().all(|(on_output, _)| on_output != id),
                "{id:?} is already on the output"
            );
            assert!(
                geo.loc.y >= OUTPUT_H,
                "{id:?} at {geo:?} is not below the output"
            );
        }
    }

    #[test]
    fn egl_render_workspace_overview_invalid_index_draws_nothing() {
        let (mut f, output) = set_up();

        // Index 4 is one past the last workspace, where the overview still has geometry.
        assert!(render(&mut f, &output, Draw::Workspace(4)).is_empty());
        assert!(render(&mut f, &output, Draw::Workspace(usize::MAX)).is_empty());
    }
}
