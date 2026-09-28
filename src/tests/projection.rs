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
fn overview_columns_zero_sources_offset_zero_when_strip_centered() {
    let viewer_size = size(2000., 1000.);
    // Strip width 800, already at its centred position, so the layout should
    // find an offset of 0.
    let strip = rect(600., 0., 800., 1000.);

    let result = overview_columns(strip, viewer_size, &[]);

    assert!(result.regions.is_empty());
    assert!(result.strip_offset_x.abs() < 1e-9);
}

#[test]
fn overview_columns_one_source_fits() {
    let viewer_size = size(3000., 1000.);
    let strip = rect(0., 0., 1000., 1000.);
    let sources = [size(800., 600.)];

    let result = overview_columns(strip, viewer_size, &sources);

    assert_eq!(result.regions.len(), 1);
    let region = result.regions[0];

    // Row = strip(1000) + gap(50) + region(800) = 1850, centred in 3000.
    assert!((result.strip_offset_x - 575.).abs() < 1e-9);
    assert!((region.size.w - 800.).abs() < 1e-9);
    assert!((region.size.h - 600.).abs() < 1e-9);

    let strip_right_edge = strip.loc.x + result.strip_offset_x + strip.size.w;
    assert!((region.loc.x - (strip_right_edge + 50.)).abs() < 1e-9);
    assert!((region.loc.y - 200.).abs() < 1e-9);
}

#[test]
fn overview_columns_three_sources_overflow_shrinks_only_sources() {
    let viewer_size = size(2000., 1000.);
    let strip = rect(0., 0., 500., 1000.);
    let sources = [size(600., 600.), size(600., 600.), size(600., 600.)];

    let result = overview_columns(strip, viewer_size, &sources);

    assert_eq!(result.regions.len(), 3);

    // Unshrunk row would be 500 + 3*50 + 1800 = 2450 > 0.95 * 2000 = 1900, so
    // sources shrink to make the row exactly 1900 wide; the strip itself
    // keeps its own width.
    let row_width = (result.regions[2].loc.x + result.regions[2].size.w)
        - (strip.loc.x + result.strip_offset_x);
    assert!((row_width - 1900.).abs() < 1e-6);

    let shrunk_w = result.regions[0].size.w;
    assert!(shrunk_w < 600.);
    for r in &result.regions {
        assert!((r.size.w - shrunk_w).abs() < 1e-9);
        assert!((r.size.h - shrunk_w).abs() < 1e-9);
    }
}

#[test]
fn overview_columns_zoom_one_places_regions_off_screen() {
    let viewer_size = size(2000., 1000.);
    let strip = rect(0., 0., 2000., 1000.);
    let sources = [size(400., 300.)];

    let result = overview_columns(strip, viewer_size, &sources);

    assert!(result.strip_offset_x.abs() < 1e-9);
    assert_eq!(result.regions.len(), 1);
    assert!(result.regions[0].loc.x >= viewer_size.w);
}
