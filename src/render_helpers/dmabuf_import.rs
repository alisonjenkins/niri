use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::{ImportDma, RendererSuper};
use tracing::debug;

/// Imports a client dmabuf to check that this GPU can use it, then frees the imports of buffers
/// that are gone.
///
/// The renderer caches every import and only drops dead ones when a frame finishes. A window on
/// an inactive workspace, or an output that is never drawn, keeps importing without any frame,
/// so each replaced buffer would stay on the GPU until something else renders.
pub fn import_dmabuf_checked<R: ImportDma>(
    renderer: &mut R,
    dmabuf: &Dmabuf,
) -> Result<(), <R as RendererSuper>::Error> {
    renderer.import_dmabuf(dmabuf, None)?;
    if let Err(err) = renderer.cleanup_texture_cache() {
        debug!("error cleaning up the texture cache: {err:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};

    use smithay::backend::allocator::dmabuf::AsDmabuf;
    use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
    use smithay::backend::allocator::{Allocator, Fourcc, Modifier};
    use smithay::backend::egl::{EGLContext, EGLDisplay};
    use smithay::backend::renderer::gles::GlesRenderer;
    use smithay::utils::DeviceFd;

    use super::*;

    const BUFFER_WIDTH: u32 = 1920;
    const BUFFER_HEIGHT: u32 = 1080;
    const BUFFER_BYTES: u64 = BUFFER_WIDTH as u64 * BUFFER_HEIGHT as u64 * 4;
    const IMPORTS: u64 = 24;

    /// Bytes this process holds in GPU memory, summed over every DRM client and memory region
    /// that `fdinfo` reports.
    fn drm_bytes_held() -> Option<u64> {
        let mut clients = std::collections::HashMap::new();
        for entry in fs::read_dir("/proc/self/fdinfo").ok()? {
            let Ok(info) = fs::read_to_string(entry.ok()?.path()) else {
                continue;
            };
            let Some(client) = info
                .lines()
                .find_map(|line| line.strip_prefix("drm-client-id:"))
            else {
                continue;
            };
            let total: u64 = info
                .lines()
                .filter_map(|line| line.strip_prefix("drm-total-"))
                .filter_map(|rest| rest.split_once(':')?.1.trim().split_once(' '))
                .filter_map(|(num, unit)| {
                    let mult = match unit {
                        "KiB" => 1024,
                        "MiB" => 1024 * 1024,
                        "GiB" => 1024 * 1024 * 1024,
                        _ => return None,
                    };
                    Some(num.parse::<u64>().ok()? * mult)
                })
                .sum();
            clients.insert(client.trim().to_owned(), total);
        }
        Some(clients.values().sum())
    }

    #[test]
    fn egl_import_without_frames_does_not_pin_dead_buffers() {
        let Ok(node) = File::options()
            .read(true)
            .write(true)
            .open("/dev/dri/renderD128")
        else {
            eprintln!("skipping: no /dev/dri/renderD128");
            return;
        };
        let gbm = GbmDevice::new(DeviceFd::from(std::os::fd::OwnedFd::from(node))).unwrap();
        let display = unsafe { EGLDisplay::new(gbm.clone()) }.unwrap();
        let context = EGLContext::new(&display).unwrap();
        let mut renderer = unsafe { GlesRenderer::new(context) }.unwrap();
        let mut allocator = GbmAllocator::new(gbm, GbmBufferFlags::RENDERING);

        let Some(before) = drm_bytes_held() else {
            eprintln!("skipping: no /proc/self/fdinfo");
            return;
        };
        if before == 0 {
            eprintln!("skipping: driver reports no drm-total fdinfo");
            return;
        }

        // A client that keeps replacing its buffers while nothing renders, like a window on an
        // inactive workspace of an output that is never drawn.
        for _ in 0..IMPORTS {
            let buffer = allocator
                .create_buffer(
                    BUFFER_WIDTH,
                    BUFFER_HEIGHT,
                    Fourcc::Argb8888,
                    &[Modifier::Linear],
                )
                .unwrap();
            let dmabuf = buffer.export().unwrap();
            import_dmabuf_checked(&mut renderer, &dmabuf).unwrap();
        }

        let held = drm_bytes_held().unwrap().saturating_sub(before);
        assert!(
            held < 4 * BUFFER_BYTES,
            "{held} bytes still held after {IMPORTS} imports of dead {BUFFER_BYTES}-byte buffers"
        );
    }
}
