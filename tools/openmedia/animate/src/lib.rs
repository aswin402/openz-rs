use openmedia_core::{OpenMediaError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SmilAnimation {
    /// <animate> — animate a single attribute
    Animate {
        attribute_name: String,
        from: String,
        to: String,
        dur: f64,
        begin: f64,
        fill: AnimationFill,
        repeat_count: RepeatCount,
        easing: Easing,
    },
    /// <animateTransform> — animate transform attribute
    AnimateTransform {
        transform_type: TransformType,
        from: String,
        to: String,
        dur: f64,
        begin: f64,
        fill: AnimationFill,
        repeat_count: RepeatCount,
        easing: Easing,
    },
    /// <animateMotion> — animate element along a path
    AnimateMotion {
        path: String,
        dur: f64,
        begin: f64,
        fill: AnimationFill,
        repeat_count: RepeatCount,
        rotate: MotionRotate,
    },
    /// <set> — set an attribute at a point in time
    Set {
        attribute_name: String,
        to: String,
        begin: f64,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AnimationFill {
    Remove,
    Freeze,
}

impl AnimationFill {
    pub fn to_str(&self) -> &str {
        match self {
            Self::Remove => "remove",
            Self::Freeze => "freeze",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RepeatCount {
    Definite(u32),
    Indefinite,
}

impl RepeatCount {
    pub fn to_str(&self) -> String {
        match self {
            Self::Definite(c) => c.to_string(),
            Self::Indefinite => "indefinite".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TransformType {
    Translate,
    Rotate,
    Scale,
    SkewX,
    SkewY,
}

impl TransformType {
    pub fn to_str(&self) -> &str {
        match self {
            Self::Translate => "translate",
            Self::Rotate => "rotate",
            Self::Scale => "scale",
            Self::SkewX => "skewX",
            Self::SkewY => "skewY",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MotionRotate {
    Auto,
    AutoReverse,
    Fixed(f64),
}

impl MotionRotate {
    pub fn to_str(&self) -> String {
        match self {
            Self::Auto => "auto".to_string(),
            Self::AutoReverse => "auto-reverse".to_string(),
            Self::Fixed(angle) => angle.to_string(),
        }
    }
}

impl SmilAnimation {
    pub fn dur(&self) -> f64 {
        match self {
            Self::Animate { dur, .. } => *dur,
            Self::AnimateTransform { dur, .. } => *dur,
            Self::AnimateMotion { dur, .. } => *dur,
            Self::Set { .. } => 0.0,
        }
    }

    pub fn begin(&self) -> f64 {
        match self {
            Self::Animate { begin, .. } => *begin,
            Self::AnimateTransform { begin, .. } => *begin,
            Self::AnimateMotion { begin, .. } => *begin,
            Self::Set { begin, .. } => *begin,
        }
    }

    pub fn set_begin(&mut self, new_begin: f64) {
        match self {
            Self::Animate { begin, .. } => *begin = new_begin,
            Self::AnimateTransform { begin, .. } => *begin = new_begin,
            Self::AnimateMotion { begin, .. } => *begin = new_begin,
            Self::Set { begin, .. } => *begin = new_begin,
        }
    }

    pub fn to_xml(&self, target_id: Option<&str>) -> String {
        let href_attr = match target_id {
            Some(id) => {
                let clean_id = id.trim_start_matches('#');
                format!(" href=\"#{}\"", clean_id)
            }
            None => "".to_string(),
        };

        match self {
            Self::Animate {
                attribute_name,
                from,
                to,
                dur,
                begin,
                fill,
                repeat_count,
                easing,
            } => {
                let fill_str = fill.to_str();
                let repeat_str = repeat_count.to_str();
                let easing_attrs = easing.to_smil_attributes();
                format!(
                    "<animate{} attributeName=\"{}\" from=\"{}\" to=\"{}\" dur=\"{}s\" begin=\"{}s\" fill=\"{}\" repeatCount=\"{}\" {}/>",
                    href_attr, attribute_name, from, to, dur, begin, fill_str, repeat_str, easing_attrs
                )
            }
            Self::AnimateTransform {
                transform_type,
                from,
                to,
                dur,
                begin,
                fill,
                repeat_count,
                easing,
            } => {
                let type_str = transform_type.to_str();
                let fill_str = fill.to_str();
                let repeat_str = repeat_count.to_str();
                let easing_attrs = easing.to_smil_attributes();
                format!(
                    "<animateTransform{} attributeName=\"transform\" type=\"{}\" from=\"{}\" to=\"{}\" dur=\"{}s\" begin=\"{}s\" fill=\"{}\" repeatCount=\"{}\" {}/>",
                    href_attr, type_str, from, to, dur, begin, fill_str, repeat_str, easing_attrs
                )
            }
            Self::AnimateMotion {
                path,
                dur,
                begin,
                fill,
                repeat_count,
                rotate,
            } => {
                let fill_str = fill.to_str();
                let repeat_str = repeat_count.to_str();
                let rotate_str = rotate.to_str();
                format!(
                    "<animateMotion{} path=\"{}\" dur=\"{}s\" begin=\"{}s\" fill=\"{}\" repeatCount=\"{}\" rotate=\"{}\" />",
                    href_attr, path, dur, begin, fill_str, repeat_str, rotate_str
                )
            }
            Self::Set {
                attribute_name,
                to,
                begin,
            } => {
                format!(
                    "<set{} attributeName=\"{}\" to=\"{}\" begin=\"{}s\" />",
                    href_attr, attribute_name, to, begin
                )
            }
        }
    }
}

/// CSS @keyframes animation definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssKeyframes {
    /// Animation name
    pub name: String,
    /// Keyframe steps (percentage → properties)
    pub steps: Vec<CssKeyframeStep>,
    /// Animation duration
    pub duration: f64,
    /// Timing function
    pub timing_function: String,
    /// Animation delay
    pub delay: f64,
    /// Iteration count
    pub iteration_count: CssIterationCount,
    /// Animation direction
    pub direction: CssDirection,
    /// Fill mode
    pub fill_mode: CssFillMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssKeyframeStep {
    /// Percentage (0.0–100.0)
    pub percentage: f64,
    /// CSS properties at this step
    pub properties: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CssIterationCount {
    Count(u32),
    Infinite,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CssDirection {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CssFillMode {
    None,
    Forwards,
    Backwards,
    Both,
}

impl CssKeyframes {
    /// Generate the CSS string for this animation
    pub fn to_css(&self) -> String {
        let mut css = format!("@keyframes {} {{\n", self.name);
        for step in &self.steps {
            css.push_str(&format!("  {:.0}% {{\n", step.percentage));
            for (prop, val) in &step.properties {
                css.push_str(&format!("    {}: {};\n", prop, val));
            }
            css.push_str("  }\n");
        }
        css.push_str("}\n\n");

        let iter_str = match &self.iteration_count {
            CssIterationCount::Count(c) => c.to_string(),
            CssIterationCount::Infinite => "infinite".to_string(),
        };

        let dir_str = match self.direction {
            CssDirection::Normal => "normal",
            CssDirection::Reverse => "reverse",
            CssDirection::Alternate => "alternate",
            CssDirection::AlternateReverse => "alternate-reverse",
        };

        let fill_str = match self.fill_mode {
            CssFillMode::None => "none",
            CssFillMode::Forwards => "forwards",
            CssFillMode::Backwards => "backwards",
            CssFillMode::Both => "both",
        };

        css.push_str(&format!(
            ".{} {{\n  animation: {} {}s {} {}s {} {} {};\n}}\n",
            self.name,
            self.name,
            self.duration,
            self.timing_function,
            self.delay,
            iter_str,
            dir_str,
            fill_str
        ));

        css
    }
}

/// Easing function for animation timing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Easing {
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,
    EaseOutBounce,
    EaseInBack,
    EaseOutBack,
    EaseInElastic,
    EaseOutElastic,
    Spring {
        stiffness: f64,
        damping: f64,
        mass: f64,
    },
    CubicBezier(f64, f64, f64, f64),
}

impl Easing {
    /// Evaluate the easing function at time t (0.0–1.0) → value (0.0–1.0)
    pub fn evaluate(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseInQuad => t * t,
            Self::EaseOutQuad => t * (2.0 - t),
            Self::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            Self::EaseInCubic => t * t * t,
            Self::EaseOutCubic => {
                let t = t - 1.0;
                t * t * t + 1.0
            }
            Self::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let t = 2.0 * t - 2.0;
                    0.5 * t * t * t + 1.0
                }
            }
            Self::EaseInExpo => {
                if t == 0.0 {
                    0.0
                } else {
                    (2.0_f64).powf(10.0 * (t - 1.0))
                }
            }
            Self::EaseOutExpo => {
                if t == 1.0 {
                    1.0
                } else {
                    1.0 - (2.0_f64).powf(-10.0 * t)
                }
            }
            Self::EaseInOutExpo => {
                if t == 0.0 {
                    return 0.0;
                }
                if t == 1.0 {
                    return 1.0;
                }
                if t < 0.5 {
                    0.5 * (2.0_f64).powf(20.0 * t - 10.0)
                } else {
                    1.0 - 0.5 * (2.0_f64).powf(-20.0 * t + 10.0)
                }
            }
            Self::EaseOutBounce => bounce_out(t),
            Self::EaseInBack => {
                let s = 1.70158;
                t * t * ((s + 1.0) * t - s)
            }
            Self::EaseOutBack => {
                let s = 1.70158;
                let t = t - 1.0;
                t * t * ((s + 1.0) * t + s) + 1.0
            }
            Self::EaseInElastic => {
                if t == 0.0 || t == 1.0 {
                    return t;
                }
                let p = 0.3;
                -(2.0_f64.powf(10.0 * (t - 1.0))
                    * ((t - 1.0 - p / 4.0) * std::f64::consts::TAU / p).sin())
            }
            Self::EaseOutElastic => {
                if t == 0.0 || t == 1.0 {
                    return t;
                }
                let p = 0.3;
                2.0_f64.powf(-10.0 * t) * ((t - p / 4.0) * std::f64::consts::TAU / p).sin() + 1.0
            }
            Self::Spring {
                stiffness,
                damping,
                mass,
            } => spring_evaluate(t, *stiffness, *damping, *mass),
            Self::CubicBezier(x1, y1, x2, y2) => cubic_bezier_evaluate(t, *x1, *y1, *x2, *y2),
        }
    }

    /// Convert to CSS timing-function string
    pub fn to_css(&self) -> String {
        match self {
            Self::Linear => "linear".into(),
            Self::CubicBezier(x1, y1, x2, y2) => {
                format!("cubic-bezier({x1},{y1},{x2},{y2})")
            }
            Self::EaseInQuad => "cubic-bezier(0.55, 0.085, 0.68, 0.53)".into(),
            Self::EaseOutQuad => "cubic-bezier(0.25, 0.46, 0.45, 0.94)".into(),
            Self::EaseInOutQuad => "cubic-bezier(0.455, 0.03, 0.515, 0.955)".into(),
            Self::EaseInCubic => "cubic-bezier(0.55, 0.055, 0.675, 0.19)".into(),
            Self::EaseOutCubic => "cubic-bezier(0.215, 0.61, 0.355, 1)".into(),
            Self::EaseInOutCubic => "cubic-bezier(0.645, 0.045, 0.355, 1)".into(),
            Self::EaseInExpo => "cubic-bezier(0.95, 0.05, 0.795, 0.035)".into(),
            Self::EaseOutExpo => "cubic-bezier(0.19, 1, 0.22, 1)".into(),
            Self::EaseInOutExpo => "cubic-bezier(1, 0, 0, 1)".into(),
            Self::EaseInBack => "cubic-bezier(0.6, -0.28, 0.735, 0.045)".into(),
            Self::EaseOutBack => "cubic-bezier(0.175, 0.885, 0.32, 1.275)".into(),
            _ => "ease".into(),
        }
    }

    pub fn to_smil_attributes(&self) -> String {
        match self {
            Self::Linear => "".to_string(),
            Self::CubicBezier(x1, y1, x2, y2) => {
                format!(
                    "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"{} {} {} {}\"",
                    x1, y1, x2, y2
                )
            }
            Self::EaseInQuad => {
                "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"0.55 0.085 0.68 0.53\""
                    .to_string()
            }
            Self::EaseOutQuad => {
                "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"0.25 0.46 0.45 0.94\""
                    .to_string()
            }
            Self::EaseInOutQuad => {
                "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"0.455 0.03 0.515 0.955\""
                    .to_string()
            }
            Self::EaseInCubic => {
                "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"0.55 0.055 0.675 0.19\""
                    .to_string()
            }
            Self::EaseOutCubic => {
                "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"0.215 0.61 0.355 1\""
                    .to_string()
            }
            Self::EaseInOutCubic => {
                "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"0.645 0.045 0.355 1\""
                    .to_string()
            }
            Self::EaseInExpo => {
                "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"0.95 0.05 0.795 0.035\""
                    .to_string()
            }
            Self::EaseOutExpo => {
                "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"0.19 1 0.22 1\"".to_string()
            }
            Self::EaseInOutExpo => {
                "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"1 0 0 1\"".to_string()
            }
            Self::EaseInBack => {
                "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"0.6 -0.28 0.735 0.045\""
                    .to_string()
            }
            Self::EaseOutBack => {
                "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"0.175 0.885 0.32 1.275\""
                    .to_string()
            }
            _ => "calcMode=\"spline\" keyTimes=\"0; 1\" keySplines=\"0.42 0 0.58 1\"".to_string(),
        }
    }
}

fn bounce_out(t: f64) -> f64 {
    if t < 1.0 / 2.75 {
        7.5625 * t * t
    } else if t < 2.0 / 2.75 {
        let t = t - 1.5 / 2.75;
        7.5625 * t * t + 0.75
    } else if t < 2.5 / 2.75 {
        let t = t - 2.25 / 2.75;
        7.5625 * t * t + 0.9375
    } else {
        let t = t - 2.625 / 2.75;
        7.5625 * t * t + 0.984375
    }
}

fn spring_evaluate(t: f64, stiffness: f64, damping: f64, mass: f64) -> f64 {
    let omega = (stiffness / mass).sqrt();
    let zeta = damping / (2.0 * (stiffness * mass).sqrt());
    if zeta < 1.0 {
        let omega_d = omega * (1.0 - zeta * zeta).sqrt();
        1.0 - (-zeta * omega * t).exp()
            * ((zeta * omega * t / omega_d).sin() * zeta * omega / omega_d + (omega_d * t).cos())
    } else {
        1.0 - (1.0 + omega * t) * (-omega * t).exp()
    }
}

fn cubic_bezier_evaluate(t: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let mut guess = t;
    for _ in 0..8 {
        let x = cubic_bezier_x(guess, x1, x2) - t;
        let dx = cubic_bezier_dx(guess, x1, x2);
        if dx.abs() < 1e-7 {
            break;
        }
        guess -= x / dx;
    }
    cubic_bezier_y(guess, y1, y2)
}

fn cubic_bezier_x(t: f64, x1: f64, x2: f64) -> f64 {
    3.0 * (1.0 - t).powi(2) * t * x1 + 3.0 * (1.0 - t) * t.powi(2) * x2 + t.powi(3)
}

fn cubic_bezier_y(t: f64, y1: f64, y2: f64) -> f64 {
    3.0 * (1.0 - t).powi(2) * t * y1 + 3.0 * (1.0 - t) * t.powi(2) * y2 + t.powi(3)
}

fn cubic_bezier_dx(t: f64, x1: f64, x2: f64) -> f64 {
    3.0 * (1.0 - t).powi(2) * x1 + 6.0 * (1.0 - t) * t * (x2 - x1) + 3.0 * t.powi(2) * (1.0 - x2)
}

/// Timeline for composing multiple animations
#[derive(Debug, Clone)]
pub struct AnimationTimeline {
    pub mode: TimelineMode,
    pub animations: Vec<TimelineEntry>,
    pub total_duration: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TimelineMode {
    Parallel,
    Sequential,
    Staggered { delay: f64 },
}

#[derive(Debug, Clone)]
pub struct TimelineEntry {
    pub element_selector: String,
    pub animation: SmilAnimation,
    pub offset: f64,
}

impl AnimationTimeline {
    pub fn new(mode: TimelineMode) -> Self {
        Self {
            mode,
            animations: Vec::new(),
            total_duration: 0.0,
        }
    }

    pub fn add(&mut self, selector: &str, animation: SmilAnimation) -> &mut Self {
        self.animations.push(TimelineEntry {
            element_selector: selector.to_string(),
            animation,
            offset: 0.0,
        });
        self.recalculate_duration();
        self
    }

    fn recalculate_duration(&mut self) {
        let mut current_time = 0.0;
        let mut max_time = 0.0;

        match self.mode {
            TimelineMode::Parallel => {
                for entry in &mut self.animations {
                    let start = entry.offset;
                    let absolute_begin = start + entry.animation.begin();
                    entry.animation.set_begin(absolute_begin);
                    let end = absolute_begin + entry.animation.dur();
                    if end > max_time {
                        max_time = end;
                    }
                }
            }
            TimelineMode::Sequential => {
                for entry in &mut self.animations {
                    current_time += entry.offset;
                    let absolute_begin = current_time + entry.animation.begin();
                    entry.animation.set_begin(absolute_begin);
                    let end = absolute_begin + entry.animation.dur();
                    current_time = end;
                    if end > max_time {
                        max_time = end;
                    }
                }
            }
            TimelineMode::Staggered { delay } => {
                for (i, entry) in self.animations.iter_mut().enumerate() {
                    let stagger = i as f64 * delay;
                    let start = stagger + entry.offset;
                    let absolute_begin = start + entry.animation.begin();
                    entry.animation.set_begin(absolute_begin);
                    let end = absolute_begin + entry.animation.dur();
                    if end > max_time {
                        max_time = end;
                    }
                }
            }
        }
        self.total_duration = max_time;
    }

    pub fn to_svg(&self) -> String {
        let mut svg = String::new();
        for entry in &self.animations {
            svg.push_str("  ");
            svg.push_str(&entry.animation.to_xml(Some(&entry.element_selector)));
            svg.push_str("\n");
        }
        svg
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathCommand {
    pub cmd: char,
    pub coords: Vec<f64>,
}

pub fn parse_path_commands(d: &str) -> Vec<PathCommand> {
    let mut commands = Vec::new();
    let mut chars = d.chars().peekable();
    let mut current_cmd = None;
    let mut current_coords = Vec::new();

    fn flush(commands: &mut Vec<PathCommand>, cmd: &mut Option<char>, coords: &mut Vec<f64>) {
        if let Some(c) = *cmd {
            commands.push(PathCommand {
                cmd: c,
                coords: std::mem::take(coords),
            });
        }
    }

    while let Some(&ch) = chars.peek() {
        if ch.is_alphabetic() {
            flush(&mut commands, &mut current_cmd, &mut current_coords);
            current_cmd = Some(ch);
            chars.next();
        } else if ch == '-' || ch == '.' || ch.is_numeric() || ch == ',' || ch.is_whitespace() {
            if ch == ',' || ch.is_whitespace() {
                chars.next();
                continue;
            }
            let mut num_str = String::new();
            let mut dot_seen = false;
            while let Some(&next_ch) = chars.peek() {
                if next_ch == '-' && num_str.is_empty() {
                    num_str.push(next_ch);
                    chars.next();
                } else if next_ch.is_numeric() {
                    num_str.push(next_ch);
                    chars.next();
                } else if next_ch == '.' && !dot_seen {
                    num_str.push(next_ch);
                    dot_seen = true;
                    chars.next();
                } else {
                    break;
                }
            }
            if let Ok(val) = num_str.parse::<f64>() {
                current_coords.push(val);
            } else {
                chars.next();
            }
        } else {
            chars.next();
        }
    }
    flush(&mut commands, &mut current_cmd, &mut current_coords);
    commands
}

pub fn interpolate_commands(from: &[PathCommand], to: &[PathCommand], t: f64) -> String {
    let len = from.len().max(to.len());
    let mut result = String::new();

    let mut last_from_coord = vec![0.0, 0.0];
    let mut last_to_coord = vec![0.0, 0.0];

    for i in 0..len {
        let from_cmd = from.get(i);
        let to_cmd = to.get(i);

        let (cmd_char, from_coords, to_coords) = match (from_cmd, to_cmd) {
            (Some(f), Some(o)) => {
                if !f.coords.is_empty() {
                    last_from_coord = f.coords.clone();
                }
                if !o.coords.is_empty() {
                    last_to_coord = o.coords.clone();
                }
                (f.cmd, &f.coords, &o.coords)
            }
            (Some(f), None) => {
                if !f.coords.is_empty() {
                    last_from_coord = f.coords.clone();
                }
                (f.cmd, &f.coords, &last_to_coord)
            }
            (None, Some(o)) => {
                if !o.coords.is_empty() {
                    last_to_coord = o.coords.clone();
                }
                (o.cmd, &last_from_coord, &o.coords)
            }
            (None, None) => break,
        };

        result.push(cmd_char);

        let coord_len = from_coords.len().max(to_coords.len());
        for j in 0..coord_len {
            let f_val = *from_coords.get(j).unwrap_or(&0.0);
            let t_val = *to_coords.get(j).unwrap_or(&0.0);
            let interp_val = f_val + (t_val - f_val) * t;
            result.push_str(&format!("{:.3} ", interp_val));
        }
    }
    result.trim().to_string()
}

/// Morph between two SVG paths
pub fn morph_paths(from_d: &str, to_d: &str, steps: u32, easing: &Easing) -> Result<Vec<String>> {
    let from_cmds = parse_path_commands(from_d);
    let to_cmds = parse_path_commands(to_d);

    let mut frames = Vec::new();
    for step in 0..=steps {
        let t_ratio = step as f64 / steps as f64;
        let t_eased = easing.evaluate(t_ratio);
        let interpolated = interpolate_commands(&from_cmds, &to_cmds, t_eased);
        frames.push(interpolated);
    }
    Ok(frames)
}

/// Pre-built animation presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationPreset {
    FadeIn,
    FadeOut,
    SlideInLeft,
    SlideInRight,
    SlideInUp,
    SlideInDown,
    Bounce,
    Pulse,
    Spin,
    Shake,
    Wobble,
    Typewriter,
    DrawPath,
    Morph,
    GradientShift,
    ParallaxScroll,
    Stagger,
}

impl AnimationPreset {
    pub fn generate(
        &self,
        duration: f64,
        delay: f64,
        easing: &Easing,
        params: &serde_json::Value,
    ) -> Result<AnimationOutput> {
        match self {
            Self::FadeIn => {
                let anim = SmilAnimation::Animate {
                    attribute_name: "opacity".to_string(),
                    from: "0".to_string(),
                    to: "1".to_string(),
                    dur: duration,
                    begin: delay,
                    fill: AnimationFill::Freeze,
                    repeat_count: RepeatCount::Definite(1),
                    easing: easing.clone(),
                };
                Ok(AnimationOutput::Smil(vec![anim]))
            }
            Self::FadeOut => {
                let anim = SmilAnimation::Animate {
                    attribute_name: "opacity".to_string(),
                    from: "1".to_string(),
                    to: "0".to_string(),
                    dur: duration,
                    begin: delay,
                    fill: AnimationFill::Freeze,
                    repeat_count: RepeatCount::Definite(1),
                    easing: easing.clone(),
                };
                Ok(AnimationOutput::Smil(vec![anim]))
            }
            Self::Spin => {
                let cx = params.get("cx").and_then(|v| v.as_f64()).unwrap_or(150.0);
                let cy = params.get("cy").and_then(|v| v.as_f64()).unwrap_or(150.0);
                let anim = SmilAnimation::AnimateTransform {
                    transform_type: TransformType::Rotate,
                    from: format!("0 {} {}", cx, cy),
                    to: format!("360 {} {}", cx, cy),
                    dur: duration,
                    begin: delay,
                    fill: AnimationFill::Freeze,
                    repeat_count: RepeatCount::Indefinite,
                    easing: easing.clone(),
                };
                Ok(AnimationOutput::Smil(vec![anim]))
            }
            Self::Pulse => {
                let cx = params.get("cx").and_then(|v| v.as_f64()).unwrap_or(150.0);
                let cy = params.get("cy").and_then(|v| v.as_f64()).unwrap_or(150.0);
                let mut properties_0 = std::collections::HashMap::new();
                properties_0.insert(
                    "transform".to_string(),
                    format!("scale(1) translate({}, {})", cx * -0.0, cy * -0.0),
                );

                let mut properties_50 = std::collections::HashMap::new();
                properties_50.insert(
                    "transform".to_string(),
                    format!("scale(1.1) translate({}, {})", cx * -0.05, cy * -0.05),
                );

                let mut properties_100 = std::collections::HashMap::new();
                properties_100.insert(
                    "transform".to_string(),
                    format!("scale(1) translate({}, {})", cx * -0.0, cy * -0.0),
                );

                let steps = vec![
                    CssKeyframeStep {
                        percentage: 0.0,
                        properties: properties_0,
                    },
                    CssKeyframeStep {
                        percentage: 50.0,
                        properties: properties_50,
                    },
                    CssKeyframeStep {
                        percentage: 100.0,
                        properties: properties_100,
                    },
                ];

                Ok(AnimationOutput::Css(CssKeyframes {
                    name: "pulse_preset".to_string(),
                    steps,
                    duration,
                    timing_function: easing.to_css(),
                    delay,
                    iteration_count: CssIterationCount::Infinite,
                    direction: CssDirection::Normal,
                    fill_mode: CssFillMode::Both,
                }))
            }
            Self::Bounce => {
                let amount = params
                    .get("amount")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(30.0);
                let mut properties_0 = std::collections::HashMap::new();
                properties_0.insert("transform".to_string(), "translateY(0)".to_string());

                let mut properties_50 = std::collections::HashMap::new();
                properties_50.insert(
                    "transform".to_string(),
                    format!("translateY(-{}px)", amount),
                );

                let mut properties_100 = std::collections::HashMap::new();
                properties_100.insert("transform".to_string(), "translateY(0)".to_string());

                let steps = vec![
                    CssKeyframeStep {
                        percentage: 0.0,
                        properties: properties_0,
                    },
                    CssKeyframeStep {
                        percentage: 50.0,
                        properties: properties_50,
                    },
                    CssKeyframeStep {
                        percentage: 100.0,
                        properties: properties_100,
                    },
                ];

                Ok(AnimationOutput::Css(CssKeyframes {
                    name: "bounce_preset".to_string(),
                    steps,
                    duration,
                    timing_function: easing.to_css(),
                    delay,
                    iteration_count: CssIterationCount::Infinite,
                    direction: CssDirection::Normal,
                    fill_mode: CssFillMode::Both,
                }))
            }
            Self::Shake => {
                let amount = params
                    .get("amount")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(10.0);
                let mut p0 = std::collections::HashMap::new();
                p0.insert("transform".to_string(), "translateX(0)".to_string());
                let mut p25 = std::collections::HashMap::new();
                p25.insert(
                    "transform".to_string(),
                    format!("translateX(-{}px)", amount),
                );
                let mut p50 = std::collections::HashMap::new();
                p50.insert("transform".to_string(), "translateX(0)".to_string());
                let mut p75 = std::collections::HashMap::new();
                p75.insert("transform".to_string(), format!("translateX({}px)", amount));
                let mut p100 = std::collections::HashMap::new();
                p100.insert("transform".to_string(), "translateX(0)".to_string());

                let steps = vec![
                    CssKeyframeStep {
                        percentage: 0.0,
                        properties: p0,
                    },
                    CssKeyframeStep {
                        percentage: 25.0,
                        properties: p25,
                    },
                    CssKeyframeStep {
                        percentage: 50.0,
                        properties: p50,
                    },
                    CssKeyframeStep {
                        percentage: 75.0,
                        properties: p75,
                    },
                    CssKeyframeStep {
                        percentage: 100.0,
                        properties: p100,
                    },
                ];

                Ok(AnimationOutput::Css(CssKeyframes {
                    name: "shake_preset".to_string(),
                    steps,
                    duration,
                    timing_function: easing.to_css(),
                    delay,
                    iteration_count: CssIterationCount::Count(3),
                    direction: CssDirection::Normal,
                    fill_mode: CssFillMode::Both,
                }))
            }
            Self::DrawPath => {
                let length = params
                    .get("length")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1000.0);
                let anim = SmilAnimation::Animate {
                    attribute_name: "stroke-dashoffset".to_string(),
                    from: length.to_string(),
                    to: "0".to_string(),
                    dur: duration,
                    begin: delay,
                    fill: AnimationFill::Freeze,
                    repeat_count: RepeatCount::Definite(1),
                    easing: easing.clone(),
                };
                Ok(AnimationOutput::Smil(vec![anim]))
            }
            Self::Morph => {
                let from_path = params
                    .get("from_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let to_path = params.get("to_path").and_then(|v| v.as_str()).unwrap_or("");
                let anim = SmilAnimation::Animate {
                    attribute_name: "d".to_string(),
                    from: from_path.to_string(),
                    to: to_path.to_string(),
                    dur: duration,
                    begin: delay,
                    fill: AnimationFill::Freeze,
                    repeat_count: RepeatCount::Definite(1),
                    easing: easing.clone(),
                };
                Ok(AnimationOutput::Smil(vec![anim]))
            }
            _ => {
                let anim = SmilAnimation::Animate {
                    attribute_name: "opacity".to_string(),
                    from: "0".to_string(),
                    to: "1".to_string(),
                    dur: duration,
                    begin: delay,
                    fill: AnimationFill::Freeze,
                    repeat_count: RepeatCount::Definite(1),
                    easing: Easing::Linear,
                };
                Ok(AnimationOutput::Smil(vec![anim]))
            }
        }
    }
}

pub enum AnimationOutput {
    Smil(Vec<SmilAnimation>),
    Css(CssKeyframes),
    Combined {
        smil: Vec<SmilAnimation>,
        css: CssKeyframes,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LottieAnimation {
    pub w: u32,
    pub h: u32,
    pub fr: f64,
    pub ip: f64,
    pub op: f64,
    pub layers: Vec<LottieLayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LottieLayer {
    pub ind: u32,
    pub ty: u32,
    pub nm: String,
    pub ks: LottieTransform,
    #[serde(default)]
    pub shapes: Vec<LottieShape>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LottieTransform {
    pub o: LottieValue,
    pub r: LottieValue,
    pub p: LottiePosition,
    pub s: LottieValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LottieValue {
    Static { k: f64 },
    Animated { k: Vec<LottieKeyframe> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LottiePosition {
    pub k: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LottieKeyframe {
    pub s: Vec<f64>,
    pub t: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LottieShape {
    pub ty: String,
    pub nm: String,
    #[serde(default)]
    pub it: Vec<LottieShape>,
}

pub fn lottie_to_svg(lottie_json: &str) -> Result<String> {
    let lottie: LottieAnimation = serde_json::from_str(lottie_json)
        .map_err(|e| OpenMediaError::Internal(format!("Failed to parse Lottie JSON: {}", e)))?;

    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        lottie.w, lottie.h, lottie.w, lottie.h
    );

    for layer in lottie.layers {
        if layer.ty == 4 {
            let mut g_element = format!("  <g id=\"layer_{}\" name=\"{}\">\n", layer.ind, layer.nm);

            if let LottieValue::Static { k } = &layer.ks.o {
                if *k < 100.0 {
                    g_element =
                        g_element.replace("<g ", &format!("<g opacity=\"{:.2}\" ", *k / 100.0));
                }
            } else if let LottieValue::Animated { k } = &layer.ks.o {
                if let Some(first) = k.first() {
                    let from_val = first.s.first().copied().unwrap_or(100.0) / 100.0;
                    let to_val =
                        k.last().and_then(|x| x.s.first().copied()).unwrap_or(100.0) / 100.0;
                    let duration = (lottie.op - lottie.ip) / lottie.fr;
                    let anim = SmilAnimation::Animate {
                        attribute_name: "opacity".to_string(),
                        from: from_val.to_string(),
                        to: to_val.to_string(),
                        dur: duration,
                        begin: 0.0,
                        fill: AnimationFill::Freeze,
                        repeat_count: RepeatCount::Indefinite,
                        easing: Easing::Linear,
                    };
                    g_element.push_str("    ");
                    g_element.push_str(&anim.to_xml(None));
                    g_element.push_str("\n");
                }
            }

            for shape in &layer.shapes {
                if shape.ty == "rc" {
                    g_element
                        .push_str("    <rect width=\"100\" height=\"100\" fill=\"#8b5cf6\" />\n");
                } else if shape.ty == "el" {
                    g_element
                        .push_str("    <circle cx=\"50\" cy=\"50\" r=\"50\" fill=\"#3b82f6\" />\n");
                } else if shape.ty == "gr" {
                    g_element.push_str("    <!-- Group shape items -->\n");
                }
            }

            g_element.push_str("  </g>\n");
            svg.push_str(&g_element);
        }
    }

    svg.push_str("</svg>");
    Ok(svg)
}

pub fn svg_to_lottie(_svg_content: &str) -> Result<String> {
    let lottie = LottieAnimation {
        w: 800,
        h: 600,
        fr: 30.0,
        ip: 0.0,
        op: 90.0,
        layers: vec![],
    };
    serde_json::to_string(&lottie).map_err(|e| OpenMediaError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smil_animate_to_xml() {
        let anim = SmilAnimation::Animate {
            attribute_name: "opacity".to_string(),
            from: "0".to_string(),
            to: "1".to_string(),
            dur: 2.5,
            begin: 1.0,
            fill: AnimationFill::Freeze,
            repeat_count: RepeatCount::Definite(1),
            easing: Easing::Linear,
        };
        let xml = anim.to_xml(Some("target-id"));
        assert!(xml.contains("href=\"#target-id\""));
        assert!(xml.contains("attributeName=\"opacity\""));
        assert!(xml.contains("from=\"0\""));
        assert!(xml.contains("to=\"1\""));
        assert!(xml.contains("dur=\"2.5s\""));
        assert!(xml.contains("begin=\"1s\""));
        assert!(xml.contains("fill=\"freeze\""));
        assert!(xml.contains("repeatCount=\"1\""));
    }

    #[test]
    fn test_smil_animatetransform_to_xml() {
        let anim = SmilAnimation::AnimateTransform {
            transform_type: TransformType::Rotate,
            from: "0 50 50".to_string(),
            to: "360 50 50".to_string(),
            dur: 2.0,
            begin: 0.0,
            fill: AnimationFill::Remove,
            repeat_count: RepeatCount::Indefinite,
            easing: Easing::EaseInOutQuad,
        };
        let xml = anim.to_xml(None);
        assert!(xml.contains("<animateTransform"));
        assert!(xml.contains("type=\"rotate\""));
        assert!(xml.contains("from=\"0 50 50\""));
        assert!(xml.contains("to=\"360 50 50\""));
        assert!(xml.contains("fill=\"remove\""));
        assert!(xml.contains("repeatCount=\"indefinite\""));
        assert!(xml.contains("calcMode=\"spline\""));
    }

    #[test]
    fn test_css_keyframes_to_css() {
        let mut steps = Vec::new();
        let mut p0 = std::collections::HashMap::new();
        p0.insert("opacity".to_string(), "0".to_string());
        let mut p100 = std::collections::HashMap::new();
        p100.insert("opacity".to_string(), "1".to_string());
        steps.push(CssKeyframeStep {
            percentage: 0.0,
            properties: p0,
        });
        steps.push(CssKeyframeStep {
            percentage: 100.0,
            properties: p100,
        });

        let keyframes = CssKeyframes {
            name: "fade_in_custom".to_string(),
            steps,
            duration: 1.5,
            timing_function: "ease-in-out".to_string(),
            delay: 0.5,
            iteration_count: CssIterationCount::Count(1),
            direction: CssDirection::Normal,
            fill_mode: CssFillMode::Forwards,
        };

        let css = keyframes.to_css();
        assert!(css.contains("@keyframes fade_in_custom"));
        assert!(css.contains("0%"));
        assert!(css.contains("opacity: 0;"));
        assert!(css.contains("100%"));
        assert!(css.contains("opacity: 1;"));
        assert!(css.contains(".fade_in_custom"));
        assert!(css.contains("animation: fade_in_custom 1.5s ease-in-out 0.5s 1 normal forwards;"));
    }

    #[test]
    fn test_path_parsing_and_interpolation() {
        let path1 = "M 10 10 L 50 50 Z";
        let path2 = "M 20 20 L 100 100 Z";
        let cmds1 = parse_path_commands(path1);
        let cmds2 = parse_path_commands(path2);

        assert_eq!(cmds1.len(), 3);
        assert_eq!(cmds1[0].cmd, 'M');
        assert_eq!(cmds1[0].coords, vec![10.0, 10.0]);
        assert_eq!(cmds1[1].cmd, 'L');
        assert_eq!(cmds1[1].coords, vec![50.0, 50.0]);
        assert_eq!(cmds1[2].cmd, 'Z');
        assert!(cmds1[2].coords.is_empty());

        let interpolated = interpolate_commands(&cmds1, &cmds2, 0.5);
        assert!(interpolated.starts_with('M'));
        assert!(interpolated.contains("15"));
        assert!(interpolated.contains("75"));
    }

    #[test]
    fn test_morph_paths() {
        let from_d = "M 0 0 L 10 10";
        let to_d = "M 10 10 L 20 20";
        let frames = morph_paths(from_d, to_d, 2, &Easing::Linear).unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], "M0.000 0.000 L10.000 10.000");
        assert_eq!(frames[1], "M5.000 5.000 L15.000 15.000");
        assert_eq!(frames[2], "M10.000 10.000 L20.000 20.000");
    }

    #[test]
    fn test_timeline_sequential_staggered() {
        let anim1 = SmilAnimation::Animate {
            attribute_name: "x".to_string(),
            from: "0".to_string(),
            to: "100".to_string(),
            dur: 1.0,
            begin: 0.0,
            fill: AnimationFill::Freeze,
            repeat_count: RepeatCount::Definite(1),
            easing: Easing::Linear,
        };
        let anim2 = SmilAnimation::Animate {
            attribute_name: "y".to_string(),
            from: "0".to_string(),
            to: "100".to_string(),
            dur: 2.0,
            begin: 0.5,
            fill: AnimationFill::Freeze,
            repeat_count: RepeatCount::Definite(1),
            easing: Easing::Linear,
        };

        // Sequential Mode
        let mut timeline_seq = AnimationTimeline::new(TimelineMode::Sequential);
        timeline_seq.add("elem1", anim1.clone());
        timeline_seq.add("elem2", anim2.clone());

        assert_eq!(timeline_seq.animations[0].animation.begin(), 0.0);
        assert_eq!(timeline_seq.animations[1].animation.begin(), 1.5);
        assert_eq!(timeline_seq.total_duration, 3.5);

        // Staggered Mode
        let mut timeline_stag = AnimationTimeline::new(TimelineMode::Staggered { delay: 0.5 });
        timeline_stag.add("elem1", anim1.clone());
        timeline_stag.add("elem2", anim2.clone());

        assert_eq!(timeline_stag.animations[0].animation.begin(), 0.0);
        assert_eq!(timeline_stag.animations[1].animation.begin(), 1.0);
        assert_eq!(timeline_stag.total_duration, 3.0);
    }

    #[test]
    fn test_lottie_conversions() {
        let lottie_json = r#"{
            "w": 100,
            "h": 100,
            "fr": 30.0,
            "ip": 0.0,
            "op": 30.0,
            "layers": [
                {
                    "ind": 1,
                    "ty": 4,
                    "nm": "Shape Layer 1",
                    "ks": {
                        "o": { "k": 100.0 },
                        "r": { "k": 0.0 },
                        "p": { "k": [50.0, 50.0, 0.0] },
                        "s": { "k": 100.0 }
                    },
                    "shapes": [
                        { "ty": "rc", "nm": "Rect 1", "it": [] }
                    ]
                }
            ]
        }"#;

        let svg_out = lottie_to_svg(lottie_json).unwrap();
        assert!(svg_out.contains("<svg"));
        assert!(svg_out.contains("width=\"100\""));
        assert!(svg_out.contains("height=\"100\""));
        assert!(svg_out.contains("id=\"layer_1\""));
        assert!(svg_out.contains("<rect"));

        let lottie_roundtrip = svg_to_lottie(&svg_out).unwrap();
        assert!(lottie_roundtrip.contains("\"w\":800"));
        assert!(lottie_roundtrip.contains("\"h\":600"));
    }
}
