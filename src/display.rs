#[cfg(not(feature = "simulator"))]
use crate::memory_lcd::MemoryLcd;
#[cfg(feature = "simulator")]
use crate::memory_lcd_simulator::MemoryLcd;
use chrono::Local;
use embedded_graphics::{
    drawable::Drawable,
    fonts::{Font6x6, Font8x16, Text},
    geometry,
    geometry::Size,
    pixelcolor::BinaryColor,
    primitives::{rectangle::Rectangle, Primitive},
    style::{PrimitiveStyleBuilder, TextStyleBuilder},
    DrawTarget,
};
use std::time::{Duration, Instant};

pub struct Display {
    memory_lcd: MemoryLcd,
    workout: WorkoutDisplay,
    has_rendered: bool,
}

impl Display {
    pub fn new(start_instant: Instant) -> Display {
        let memory_lcd = MemoryLcd::new().unwrap();
        let workout = WorkoutDisplay::new(start_instant);
        Display {
            memory_lcd,
            workout,
            has_rendered: false,
        }
    }

    pub fn update_power(&mut self, power: Option<i16>) {
        self.workout.update_power(power);
    }

    pub fn update_cadence(&mut self, cadence: Option<u8>) {
        self.workout.update_cadence(cadence);
    }

    pub fn update_heart_rate(&mut self, heart_rate: Option<u8>) {
        self.workout.update_heart_rate(heart_rate);
    }

    pub fn update_external_energy(&mut self, external_energy: f64) {
        self.workout.update_external_energy(external_energy);
    }

    pub fn update_crank_count(&mut self, crank_count: u32) {
        self.workout.update_crank_count(crank_count);
    }

    pub fn update_speed(&mut self, speed: Option<f32>) {
        self.workout.update_speed(speed);
    }

    pub fn update_distance(&mut self, distance: f64) {
        self.workout.update_distance(distance);
    }

    pub fn set_gps_fix(&mut self, has_fix: bool) {
        self.workout.set_gps_fix(has_fix);
    }

    pub fn render_msg(&mut self, s: &str) {
        self.memory_lcd.clear(BinaryColor::Off).unwrap();
        self.has_rendered = false;
        MsgDisplay::new(s).draw(&mut self.memory_lcd).unwrap();
    }

    pub fn render_options(&mut self, options: &Vec<&str>) {
        // TODO: This also flickers, but stince it doesn't always
        // over draw like rendering does, it not safe to use the
        // same has_rendered approach.
        self.memory_lcd.clear(BinaryColor::Off).unwrap();
        self.has_rendered = false;
        OptionDisplay::new(&options[..])
            .draw(&mut self.memory_lcd)
            .unwrap();
    }

    pub fn render(&mut self) {
        // We only clear the screen if it's been drawing other stuff.
        // This prevents flashing or the need to frame sync.
        if !self.has_rendered {
            self.memory_lcd.clear(BinaryColor::Off).unwrap();
            self.has_rendered = true;
        }
        self.workout.clone().draw(&mut self.memory_lcd).unwrap();
    }
}

#[derive(Clone)]
pub struct WorkoutDisplay {
    power: Option<(i16, Instant)>,
    cadence: Option<(u8, Instant)>,
    heart_rate: Option<(u8, Instant)>,
    external_energy: f64,
    crank_count: Option<u32>,
    speed: Option<(f32, Instant)>,
    distance: f64,
    gps_fix: Option<(bool, Instant)>,
    start_instant: Instant,
}

impl WorkoutDisplay {
    pub fn new(start_instant: Instant) -> WorkoutDisplay {
        WorkoutDisplay {
            power: None,
            cadence: None,
            heart_rate: None,
            external_energy: 0.0,
            crank_count: None,
            speed: None,
            distance: 0.0,
            gps_fix: None,
            start_instant,
        }
    }

    pub fn update_power(&mut self, power: Option<i16>) {
        self.power = power.map(|x| (x, Instant::now()));
    }

    pub fn update_cadence(&mut self, cadence: Option<u8>) {
        self.cadence = cadence.map(|x| (x, Instant::now()));
    }

    pub fn update_heart_rate(&mut self, heart_rate: Option<u8>) {
        self.heart_rate = heart_rate.map(|x| (x, Instant::now()));
    }

    pub fn update_external_energy(&mut self, external_energy: f64) {
        self.external_energy = external_energy;
    }

    pub fn update_crank_count(&mut self, crank_count: u32) {
        self.crank_count = Some(crank_count);
    }

    pub fn update_speed(&mut self, speed: Option<f32>) {
        self.speed = speed.map(|x| (x, Instant::now()));
    }

    pub fn update_distance(&mut self, distance: f64) {
        self.distance = distance;
    }

    pub fn set_gps_fix(&mut self, has_fix: bool) {
        self.gps_fix = Some((has_fix, Instant::now()));
    }
}

impl Drawable<BinaryColor> for WorkoutDisplay {
    fn draw<D: DrawTarget<BinaryColor>>(self, target: &mut D) -> Result<(), D::Error> {
        let style_large = TextStyleBuilder::new(Font8x16)
            .text_color(BinaryColor::On)
            .background_color(BinaryColor::Off)
            .build();
        let style_tiny = TextStyleBuilder::new(Font6x6)
            .text_color(BinaryColor::On)
            .background_color(BinaryColor::Off)
            .build();

        let elapsed_secs = self.start_instant.elapsed().as_secs();
        // We lazily purge any values that are older than 5s just before render
        let power = self.power.and_then(none_if_stale);
        let cadence = self.cadence.and_then(none_if_stale);
        let heart_rate = self.heart_rate.and_then(none_if_stale);
        let speed = self.speed.and_then(none_if_stale);
        let gps_fix = self.gps_fix.and_then(none_if_stale);

        Text::new("POW (W)", geometry::Point::new(8, 8))
            .into_styled(style_tiny)
            .draw(target)?;

        Text::new(
            &power.map_or("---".to_string(), |x| format!("{:03}", x.0)),
            geometry::Point::new(8, 8 + 6),
        )
        .into_styled(style_large)
        .draw(target)?;

        Text::new("CAD (RPM)", geometry::Point::new(8, 8 + 6 + 16 + 2))
            .into_styled(style_tiny)
            .draw(target)?;

        Text::new(
            &cadence.map_or("---".to_string(), |x| format!("{:03}", x.0)),
            geometry::Point::new(8, 8 + 6 + 16 + 2 + 6),
        )
        .into_styled(style_large)
        .draw(target)?;

        Text::new(
            "HR (BPM)",
            geometry::Point::new(8, 8 + 6 + 16 + 2 + 6 + 16 + 2),
        )
        .into_styled(style_tiny)
        .draw(target)?;

        Text::new(
            &heart_rate.map_or("---".to_string(), |x| format!("{:03}", x.0)),
            geometry::Point::new(8, 8 + 6 + 16 + 2 + 6 + 16 + 2 + 6),
        )
        .into_styled(style_large)
        .draw(target)?;

        Text::new(
            "ME (KCAL)",
            geometry::Point::new(8, 8 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2),
        )
        .into_styled(style_tiny)
        .draw(target)?;

        Text::new(
            &format!(
                "{:04}",
                // We just assume 80rpm to get crank revolutions for now
                metabolic_cost_in_kcal(
                    self.external_energy,
                    self.crank_count.unwrap_or((elapsed_secs * 80 / 60) as u32)
                ) as u16
            ),
            geometry::Point::new(8, 8 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2 + 6),
        )
        .into_styled(style_large)
        .draw(target)?;

        Text::new(
            "V (km/h)",
            geometry::Point::new(8, 8 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2),
        )
        .into_styled(style_tiny)
        .draw(target)?;

        Text::new(
            &speed.map_or("---".to_string(), |x| {
                format!("{:.2}", x.0 * 60.0 * 60.0 / 1000.0)
            }),
            geometry::Point::new(8, 8 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2 + 6),
        )
        .into_styled(style_large)
        .draw(target)?;

        Text::new(
            "D (km)",
            geometry::Point::new(
                8,
                8 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2,
            ),
        )
        .into_styled(style_tiny)
        .draw(target)?;

        Text::new(
            &format!("{:.2}", self.distance / 1000.0),
            geometry::Point::new(
                8,
                8 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2 + 6 + 16 + 2 + 6,
            ),
        )
        .into_styled(style_large)
        .draw(target)?;

        Text::new("CURRENT", geometry::Point::new(8 + 50, 8))
            .into_styled(style_tiny)
            .draw(target)?;

        Text::new(
            &format!("{}", Local::now().format("%T")),
            geometry::Point::new(8 + 50, 8 + 6),
        )
        .into_styled(style_large)
        .draw(target)?;

        Text::new("ELAPSED", geometry::Point::new(8 + 50, 8 + 6 + 16 + 2))
            .into_styled(style_tiny)
            .draw(target)?;

        Text::new(
            &format!(
                "{}",
                &format!(
                    "{:02}:{:02}:{:02}",
                    elapsed_secs / 3600,
                    (elapsed_secs / 60) % 60,
                    elapsed_secs % 60
                )
            ),
            geometry::Point::new(8 + 50, 8 + 6 + 16 + 2 + 6),
        )
        .into_styled(style_large)
        .draw(target)?;

        Text::new(
            "GPS",
            geometry::Point::new(8 + 50, 8 + 6 + 16 + 2 + 6 + 16 + 2),
        )
        .into_styled(style_tiny)
        .draw(target)?;

        Text::new(
            &match gps_fix {
                None => "NO GPS",
                Some((false, _)) => "NO FIX",
                Some((true, _)) => "FIX",
            },
            geometry::Point::new(8 + 50, 8 + 6 + 16 + 2 + 6 + 16 + 2 + 6),
        )
        .into_styled(style_large)
        .draw(target)?;

        Rectangle::new(geometry::Point::new(187, 3), geometry::Point::new(193, 9))
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(BinaryColor::On)
                    .stroke_width(0)
                    .build(),
            )
            .draw(target)?;

        Ok(())
    }
}

pub struct MsgDisplay<'a>(&'a str);

impl<'a> MsgDisplay<'a> {
    pub fn new(msg: &'a str) -> MsgDisplay<'a> {
        MsgDisplay(msg)
    }
}

impl<'a> Drawable<BinaryColor> for MsgDisplay<'a> {
    fn draw<D: DrawTarget<BinaryColor>>(self, target: &mut D) -> Result<(), D::Error> {
        let style_large = TextStyleBuilder::new(Font8x16)
            .text_color(BinaryColor::On)
            .background_color(BinaryColor::Off)
            .build();

        let Size { height, width } = target.size();

        // TODO: Wrap Text
        let x = (width as i32 - (8 * (self.0.len() as i32))) / 2;
        let y = ((height as i32) - 16) / 2;

        Text::new(&self.0, geometry::Point::new(x, y))
            .into_styled(style_large)
            .draw(target)
    }
}

pub struct OptionDisplay<'a, 'b>(&'a [&'b str]);

impl<'a, 'b> OptionDisplay<'a, 'b> {
    pub fn new(opts: &'a [&'b str]) -> OptionDisplay<'a, 'b> {
        OptionDisplay(opts)
    }
}

impl<'a, 'b> Drawable<BinaryColor> for OptionDisplay<'a, 'b> {
    fn draw<D: DrawTarget<BinaryColor>>(self, target: &mut D) -> Result<(), D::Error> {
        let style_large = TextStyleBuilder::new(Font8x16)
            .text_color(BinaryColor::On)
            .background_color(BinaryColor::Off)
            .build();

        for i in 0..self.0.len() {
            let option_num = i + 1;
            Text::new(
                &format!("{}: {}", option_num, (self.0)[i]),
                geometry::Point::new(10, (i as i32) * 16 + 2 + 16 + 4),
            )
            .into_styled(style_large)
            .draw(target)?;

            Text::new(
                &format!("{}", option_num),
                geometry::Point::new(42 + (i as i32) * 37, 2),
            )
            .into_styled(style_large)
            .draw(target)?;
        }

        Ok(())
    }
}

fn none_if_stale<T>(x: (T, Instant)) -> Option<(T, Instant)> {
    if x.1.elapsed() > Duration::from_secs(5) {
        None
    } else {
        Some(x)
    }
}

// Since it's an estimate, we choose the low end (4.74 vs 5.05).  If we
// considered level of effort we could get a better guess of fats vs carbs
// burned.
fn metabolic_cost_in_kcal(external_energy: f64, crank_revolutions: u32) -> f64 {
    let ml_of_oxygen = 10.38 / 60.0 * external_energy + 4.9 * crank_revolutions as f64;
    ml_of_oxygen / 1000.0 * 4.74
}
