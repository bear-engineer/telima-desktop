//! 트레이 아이콘: `include_bytes!`로 내장한 PNG 선택과 디코딩.

use telima_app::view::tray::TrayIconState;

/// Linux 패널 색에 맞춘 변형. 템플릿 아이콘을 칠해 주지 않는 호스트용.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelVariant {
    Light,
    #[default]
    Dark,
}

/// 아이콘 선택 설정. `TrayHandle::new`에 넘긴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrayIcons {
    pub panel: PanelVariant,
}

/// 디코딩한 8비트 RGBA 이미지.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconError(pub String);

macro_rules! png {
    ($path:literal) => {
        include_bytes!(concat!("../../../assets/icon/tray/", $path)).as_slice()
    };
}

/// macOS 템플릿 아이콘(@2x, 36×36 — 메뉴 막대에서 18pt).
pub fn macos_png(state: TrayIconState) -> &'static [u8] {
    match state {
        TrayIconState::AllStopped => png!("macos/stopped@2x.png"),
        TrayIconState::Running => png!("macos/running@2x.png"),
        TrayIconState::Busy => png!("macos/busy@2x.png"),
        TrayIconState::Error => png!("macos/error@2x.png"),
    }
}

/// Linux 아이콘. `size`는 22 또는 24, 그 밖의 값은 24로 본다.
pub fn linux_png(state: TrayIconState, size: u32, panel: PanelVariant) -> &'static [u8] {
    use PanelVariant::{Dark, Light};
    use TrayIconState::{AllStopped, Busy, Error, Running};
    match (size == 22, state, panel) {
        (true, AllStopped, Light) => png!("linux/22/stopped-light.png"),
        (true, AllStopped, Dark) => png!("linux/22/stopped-dark.png"),
        (true, Running, Light) => png!("linux/22/running-light.png"),
        (true, Running, Dark) => png!("linux/22/running-dark.png"),
        (true, Busy, Light) => png!("linux/22/busy-light.png"),
        (true, Busy, Dark) => png!("linux/22/busy-dark.png"),
        (true, Error, Light) => png!("linux/22/error-light.png"),
        (true, Error, Dark) => png!("linux/22/error-dark.png"),
        (false, AllStopped, Light) => png!("linux/24/stopped-light.png"),
        (false, AllStopped, Dark) => png!("linux/24/stopped-dark.png"),
        (false, Running, Light) => png!("linux/24/running-light.png"),
        (false, Running, Dark) => png!("linux/24/running-dark.png"),
        (false, Busy, Light) => png!("linux/24/busy-light.png"),
        (false, Busy, Dark) => png!("linux/24/busy-dark.png"),
        (false, Error, Light) => png!("linux/24/error-light.png"),
        (false, Error, Dark) => png!("linux/24/error-dark.png"),
    }
}

pub fn decode_png(bytes: &[u8]) -> Result<Rgba, IconError> {
    let err = |e: &dyn std::fmt::Display| IconError(e.to_string());
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| err(&e))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| IconError("image too large".into()))?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).map_err(|e| err(&e))?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(IconError(format!(
            "expected 8-bit RGBA, got {:?}/{:?}",
            info.color_type, info.bit_depth
        )));
    }
    buf.truncate(info.buffer_size());
    Ok(Rgba {
        width: info.width,
        height: info.height,
        data: buf,
    })
}

/// SNI `IconPixmap`은 ARGB32(네트워크 바이트 순서)다.
pub fn rgba_to_argb(image: &Rgba) -> Vec<u8> {
    image
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[r, g, b, a]| [a, r, g, b])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATES: [TrayIconState; 4] = [
        TrayIconState::AllStopped,
        TrayIconState::Running,
        TrayIconState::Busy,
        TrayIconState::Error,
    ];

    #[test]
    fn macos_icons_are_36px_and_differ_per_state() {
        let mut seen = Vec::new();
        for state in STATES {
            let img = decode_png(macos_png(state)).unwrap();
            assert_eq!((img.width, img.height), (36, 36), "{state:?}");
            assert_eq!(img.data.len(), 36 * 36 * 4);
            assert!(
                !seen.contains(&img.data),
                "{state:?} duplicates another state"
            );
            seen.push(img.data);
        }
    }

    #[test]
    fn macos_icons_are_black_templates_with_shape_in_alpha() {
        for state in STATES {
            let img = decode_png(macos_png(state)).unwrap();
            assert!(img.data.chunks(4).any(|p| p[3] > 0), "{state:?} is empty");
            assert!(
                img.data
                    .chunks(4)
                    .filter(|p| p[3] > 0)
                    .all(|p| p[..3] == [0, 0, 0]),
                "{state:?} is not black"
            );
        }
    }

    #[test]
    fn linux_icons_exist_for_every_state_size_and_variant() {
        for state in STATES {
            for size in [22, 24] {
                for panel in [PanelVariant::Light, PanelVariant::Dark] {
                    let img = decode_png(linux_png(state, size, panel)).unwrap();
                    assert_eq!((img.width, img.height), (size, size), "{state:?} {panel:?}");
                }
            }
        }
    }

    #[test]
    fn linux_light_and_dark_variants_differ() {
        let light = decode_png(linux_png(TrayIconState::Running, 22, PanelVariant::Light)).unwrap();
        let dark = decode_png(linux_png(TrayIconState::Running, 22, PanelVariant::Dark)).unwrap();
        assert_ne!(light.data, dark.data);
    }

    #[test]
    fn default_panel_variant_is_dark() {
        assert_eq!(TrayIcons::default().panel, PanelVariant::Dark);
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode_png(b"not a png").is_err());
    }

    #[test]
    fn rgba_to_argb_reorders_channels() {
        let img = Rgba {
            width: 2,
            height: 1,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        assert_eq!(rgba_to_argb(&img), vec![4, 1, 2, 3, 8, 5, 6, 7]);
    }
}
