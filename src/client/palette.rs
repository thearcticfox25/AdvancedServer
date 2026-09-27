//! Colour lists for the palette swap shader and the colour maths of custom skins
//! (scr_palette, scr_pallete_shift, color_get_hue, color_get_value).

/// What the shader compares and draws: red, green, blue, each 0..255. Skin colours
/// are always opaque, so there is no alpha; the shader gets 1.
pub type Colours = Vec<[u8; 3]>;

/// PALLETE_DEFAULT: black replaced by black, so nothing visible changes. (The
/// original's was transparent black, which changed nothing either.)
pub fn default_colours() -> Colours {
    vec![[0, 0, 0]]
}

/// scr_palette: colours from 0xRRGGBB values.
pub fn from_rgb(colours: &[u32]) -> Colours {
    colours
        .iter()
        .map(|&rgb| {
            let [blue, green, red, _] = rgb.to_le_bytes();
            [red, green, blue]
        })
        .collect()
}

/// A channel as the 0..1 fraction the original's colour maths and the shader work in.
pub fn fraction(channel: u8) -> f64 {
    channel as f64 / 255.0
}

/// scr_pallete_shift: every colour moved in hue, saturation and value, each on a 0..255 scale.
pub fn shift(colours: &Colours, hue: f64, saturation: f64, value: f64) -> Colours {
    colours
        .iter()
        .map(|&[red, green, blue]| {
            let [old_hue, old_saturation, old_value] = rgb_to_hsv(fraction(red), fraction(green), fraction(blue));
            // hsv_to_rgb already gives whole channels within 0..255.
            hsv_to_rgb(old_hue * 255.0 + hue, old_saturation * 255.0 + saturation, old_value * 255.0 + value).map(|channel| channel as u8)
        })
        .collect()
}

/// color_get_hue of the first colour, 0..255.
pub fn first_colour_hue(colours: &Colours) -> f64 {
    runner_hsv(colours)[0]
}

/// color_get_value of the first colour, 0..255.
pub fn first_colour_value(colours: &Colours) -> f64 {
    runner_hsv(colours)[2]
}

/// The runner's Color_RGBtoHSV on make_color_rgb of the first colour.
fn runner_hsv(colours: &Colours) -> [f64; 3] {
    let [red, green, blue] = colours.first().copied().unwrap_or([0; 3]).map(fraction);
    let lowest = red.min(green).min(blue);
    let highest = red.max(green).max(blue);
    let spread = highest - lowest;
    let saturation = if highest == 0.0 { 0.0 } else { spread / highest };
    let mut hue_degrees = if saturation == 0.0 {
        0.0
    } else if red == highest {
        60.0 * (green - blue) / spread
    } else if green == highest {
        120.0 + 60.0 * (blue - red) / spread
    } else {
        240.0 + 60.0 * (red - green) / spread
    };
    if hue_degrees < 0.0 {
        hue_degrees += 360.0;
    }
    let to_byte_scale = |value: f64| value.clamp(0.0, 255.0);
    [to_byte_scale(hue_degrees * 255.0 / 360.0), to_byte_scale(saturation * 255.0), to_byte_scale(highest * 255.0)]
}

/// rgb_to_hsv in scr_pallete_shift: channels 0..1 in, hue, saturation and value 0..1 out.
fn rgb_to_hsv(red: f64, green: f64, blue: f64) -> [f64; 3] {
    let highest = red.max(green).max(blue);
    let lowest = red.min(green).min(blue);
    let spread = highest - lowest;
    if spread == 0.0 {
        return [0.0, 0.0, highest];
    }
    let channel_delta = |channel: f64| ((highest - channel) / 6.0 + spread / 2.0) / spread;
    let (delta_red, delta_green, delta_blue) = (channel_delta(red), channel_delta(green), channel_delta(blue));
    let mut hue = if red == highest {
        delta_blue - delta_green
    } else if green == highest {
        1.0 / 3.0 + delta_red - delta_blue
    } else {
        2.0 / 3.0 + delta_green - delta_red
    };
    if hue < 0.0 {
        hue += 1.0;
    }
    if hue > 1.0 {
        hue -= 1.0;
    }
    [hue, spread / highest, highest]
}

/// hsv_to_rgb in scr_pallete_shift: hue wraps at 256, the result is whole 0..255 channels.
fn hsv_to_rgb(hue: f64, saturation: f64, value: f64) -> [f64; 3] {
    // GameMaker's % keeps the sign of the left side, like Rust's %.
    let h = (hue % 256.0) / 255.0;
    let s = (saturation / 255.0).clamp(0.0, 1.0);
    let v = (value / 255.0).clamp(0.0, 1.0);
    let chroma = v * s;
    let second = chroma * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let matched = v - chroma;
    let [red, green, blue] = if h < 1.0 / 6.0 {
        [chroma, second, 0.0]
    } else if h < 2.0 / 6.0 {
        [second, chroma, 0.0]
    } else if h < 3.0 / 6.0 {
        [0.0, chroma, second]
    } else if h < 4.0 / 6.0 {
        [0.0, second, chroma]
    } else if h < 5.0 / 6.0 {
        [second, 0.0, chroma]
    } else {
        [chroma, 0.0, second]
    };
    // round() in GameMaker takes halves up.
    [red, green, blue].map(|channel| ((channel + matched) * 255.0 + 0.5).floor())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hue_of_pure_colours() {
        assert_eq!(first_colour_hue(&from_rgb(&[0xFF0000])), 0.0);
        assert_eq!(first_colour_hue(&from_rgb(&[0x00FF00])), 85.0);
        assert_eq!(first_colour_hue(&from_rgb(&[0x0000FF])), 170.0);
        assert_eq!(first_colour_value(&from_rgb(&[0x800000])), 128.0);
    }

    #[test]
    fn shifting_by_nothing_keeps_the_colours() {
        let colours = vec![[128, 0, 0], [224, 0, 0]];
        assert_eq!(shift(&colours, 0.0, 0.0, 0.0), colours);
    }

    #[test]
    fn a_hue_shift_turns_red_towards_green() {
        let shifted = shift(&from_rgb(&[0xFF0000]), 85.0, 0.0, 0.0);
        assert_eq!(shifted, from_rgb(&[0x00FF00]));
    }
}
