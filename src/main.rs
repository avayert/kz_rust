#![feature(slice_group_by)]


use core::f32::consts::PI;
use pixels::wgpu::Color;
use pixels::{Pixels, SurfaceTexture};
use raqote::Source;
use raqote::{DrawOptions, DrawTarget, PathBuilder, SolidSource, StrokeStyle};
use rdev::{
    self,
    EventType::{ButtonPress, ButtonRelease, KeyPress, KeyRelease},
};
use std::thread;
use std::{
    ops::RangeInclusive,
    sync::mpsc::{channel, Receiver},
};
use winit::event::Event;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;
use winit::{dpi::LogicalSize, event_loop::ControlFlow};


const CELL_SIZE: f32 = 60.0;

const SIZE: f32 = CELL_SIZE * 8.0;

const KEY_ANGLE: f32 = 80.0 * PI / 180.0;

const TRANSPARENT: SolidSource = SolidSource {
    r: 0x00,
    g: 0x00,
    b: 0x00,
    a: 0x00,
};
const WHITE: raqote::Source = Source::Solid(SolidSource {
    r: 0xFF,
    g: 0xFF,
    b: 0xFF,
    a: 0xFF,
});
// This is #FF7B81
const BLINK: raqote::Source = Source::Solid(SolidSource {
    r: 0xFF,
    g: 0x7B,
    b: 0x81,
    a: 0xFF,
});

// lol no `..Default::default()` in `const`
const STROKE: StrokeStyle = StrokeStyle {
    width: 3.0,
    cap: raqote::LineCap::Butt,
    join: raqote::LineJoin::Miter,
    miter_limit: 10.,
    dash_array: Vec::new(),
    dash_offset: 0.0,
};

const DRAW_OPTIONS: DrawOptions = DrawOptions {
    alpha: 1.0,
    blend_mode: raqote::BlendMode::SrcOver,
    antialias: raqote::AntialiasMode::Gray,
};

// TODO: don't hardcode the stroke values? or figure out to upscale properly?
// TODO: how do I antialias :S

fn create_mouse_outline() -> raqote::Path {
    let mut pen = PathBuilder::new();

    pen.move_to(337.672495, 7.010310000000004);
    pen.cubic_to(
        325.341995,
        9.051440000000003,
        307.582215,
        18.019935000000004,
        303.079985,
        21.102730000000005,
    );
    pen.cubic_to(
        299.1135585,
        23.81865000000001,
        297.6793885,
        27.953710000000008,
        297.6173685,
        31.57542000000001,
    );
    pen.cubic_to(
        297.2551785,
        52.725840000000005,
        302.754025,
        75.59952000000001,
        301.397915,
        96.82668000000001,
    );
    pen.cubic_to(
        300.79278500000004,
        106.29875000000001,
        297.6792085,
        122.33277000000001,
        296.4131185,
        128.94756,
    );
    pen.cubic_to(
        295.5877535,
        132.37404,
        295.16245349999997,
        135.88512500000002,
        295.1457985,
        139.4101,
    );
    pen.cubic_to(
        295.1459335,
        164.527,
        315.451015,
        184.88824,
        340.49864,
        184.888375,
    );
    pen.cubic_to(
        363.23333,
        184.86362499999998,
        381.696855,
        167.85317,
        385.41238000000004,
        145.36210499999999,
    );
    pen.cubic_to(
        390.742795,
        113.09571499999998,
        390.625525,
        77.16348999999998,
        391.405085,
        42.90426999999998,
    );
    pen.cubic_to(
        391.54740499999997,
        36.64971,
        390.43341499999997,
        31.257594999999995,
        387.2432,
        27.002824999999994,
    );
    pen.cubic_to(
        380.61277,
        16.217335000000002,
        353.877845,
        6.988205000000001,
        353.877845,
        6.988205000000001,
    );
    pen.line_to(353.877845, 10.58350999999999);
    pen.line_to(337.67249499999997, 10.58350999999999);
    pen.close();
    pen.move_to(354.28378, 10.58350999999999);
    pen.line_to(352.78378, 81.24063000000001);
    pen.cubic_to(
        352.709165,
        84.75531000000001,
        349.74846499999995,
        87.586245,
        345.97813499999995,
        87.586245,
    );
    pen.cubic_to(
        342.20781,
        87.58624500000002,
        339.24711,
        84.75531000000002,
        339.172495,
        81.24063000000002,
    );
    pen.line_to(337.672495, 10.58350999999999);
    pen.close();

    pen.finish()
}

fn create_input_listener() -> Receiver<rdev::Event> {
    let (sender, receiver) = channel();

    thread::spawn(move || {
        if let Err(error) = rdev::listen(move |event: rdev::Event| {
            match event.event_type {
                // TODO: add mouse move handler
                ButtonPress(_) | ButtonRelease(_) | KeyPress(_) | KeyRelease(_) => {
                    sender.send(event).unwrap()
                }
                _ => (),
            }
        }) {
            println!("ruh roh {:?}", error);
        }
    });

    receiver
}

// TODO: this is scuffed
const KEY_POSITIONS: [(rdev::Key, RangeInclusive<f32>, RangeInclusive<f32>); 8] = [
    (rdev::Key::KeyW, 2.5..=3.5, 0.05..=0.95),
    (rdev::Key::KeyR, 3.5..=4.5, 0.05..=0.95),
    (rdev::Key::ShiftLeft, 0.0..=1.5, 1.05..=1.95),
    (rdev::Key::KeyA, 1.5..=2.5, 1.05..=1.95),
    (rdev::Key::KeyS, 2.5..=3.5, 1.05..=1.95),
    (rdev::Key::KeyD, 3.5..=4.5, 1.05..=1.95),
    (rdev::Key::ControlLeft, 0.0..=1.5, 2.05..=2.95),
    (rdev::Key::Space, 1.5..=4.5, 2.05..=2.95),
];

struct Frame {
    target: DrawTarget,
    visual_mouse_offset: (f64, f64),
}

impl Frame {
    fn new() -> Self {
        Frame {
            target: DrawTarget::new(SIZE as i32, SIZE as i32),
            visual_mouse_offset: (0.0, 0.0),
        }
    }

    fn data(&self) -> &[u32] {
        self.target.get_data()
    }

    fn draw_keys(&mut self, keys: &Vec<rdev::Key>) {
        self.target.clear(TRANSPARENT);

        for (key, width, height) in KEY_POSITIONS.iter() {
            let key_offset: f32 = CELL_SIZE / KEY_ANGLE.tan(); // mfw not const

            // TODO add rounded corners by doing bezier magic
            let mut region = PathBuilder::new();

            let left = width.start() * CELL_SIZE;
            let right = width.end() * CELL_SIZE;
            let top = height.start() * CELL_SIZE;
            let bottom = height.end() * CELL_SIZE;

            let bezier_offset = CELL_SIZE * 0.05;

            // TODO: all this math is wrong and guesstimations, figure out a better approach.
            region.move_to(left + key_offset + bezier_offset, top);

            region.line_to(right - bezier_offset, top);

            region.cubic_to(
                right - bezier_offset,
                top,
                right,
                top + bezier_offset,
                right,
                top + bezier_offset * 2.0,
            );

            region.line_to(right - key_offset + bezier_offset, bottom - bezier_offset);

            region.cubic_to(
                right - key_offset + bezier_offset,
                bottom - bezier_offset,
                right - key_offset,
                bottom,
                right - key_offset - bezier_offset,
                bottom,
            );

            region.line_to(left + bezier_offset, bottom);

            region.cubic_to(
                left + bezier_offset,
                bottom,
                left,
                bottom - bezier_offset,
                left,
                bottom - bezier_offset * 2.0,
            );

            region.line_to(left + key_offset - bezier_offset, top + bezier_offset);

            region.cubic_to(
                left + key_offset - bezier_offset,
                top + bezier_offset,
                left + key_offset,
                top,
                left + key_offset + bezier_offset,
                top,
            );

            region.close();

            let path = region.finish();

            if keys.contains(key) {
                self.target.fill(&path, &BLINK, &DRAW_OPTIONS);
            }

            self.target.stroke(&path, &WHITE, &STROKE, &DRAW_OPTIONS);
        }
    }

    fn draw_mouse(&mut self, mouse_buttons: &[bool; 2]) {
        let mut left_button = PathBuilder::new();
        left_button.move_to(337.392625, 6.805049999999994);
        left_button.line_to(338.911615, 71.68908999999996);
        left_button.line_to(314.29580999999996, 84.22666499999997);
        left_button.line_to(297.6866255, 38.21717499999997);
        left_button.cubic_to(
            297.16691249999997,
            28.29091499999997,
            297.27671549999997,
            24.187329999999967,
            305.63086,
            19.279469999999968,
        );
        left_button.cubic_to(
            314.398245,
            14.128844999999975,
            326.87210999999996,
            8.901269999999975,
            337.392625,
            6.805049999999975,
        );
        left_button.close();

        let mut right_button = PathBuilder::new();
        right_button.move_to(353.877845, 6.988204999999994);
        right_button.line_to(352.990465, 71.45353499999999);
        right_button.line_to(376.97466, 84.22666499999997);
        right_button.line_to(390.72862499999997, 40.57305999999997);
        right_button.cubic_to(
            393.16090999999994,
            22.23138499999997,
            368.39287499999995,
            12.920914999999969,
            353.877845,
            6.988204999999972,
        );
        right_button.close();

        let left_path = left_button.finish();
        let right_path = right_button.finish();

        self.target
            .stroke(&create_mouse_outline(), &WHITE, &STROKE, &DRAW_OPTIONS);

        if mouse_buttons[0] {
            self.target.fill(&left_path, &BLINK, &DRAW_OPTIONS);
        }

        if mouse_buttons[1] {
            self.target.fill(&right_path, &BLINK, &DRAW_OPTIONS);
        }

        self.target
            .stroke(&left_path, &WHITE, &STROKE, &DRAW_OPTIONS);
        self.target
            .stroke(&right_path, &WHITE, &STROKE, &DRAW_OPTIONS);
    }
}

fn main() {
    let receiver = create_input_listener();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_resizable(false)
        .with_transparent(true)
        .with_title("kz")
        .with_inner_size(LogicalSize::new(SIZE, SIZE / 2.0))
        .build(&event_loop)
        .unwrap();

    let surface = SurfaceTexture::new(SIZE as u32, (SIZE / 2.0) as u32, &window);
    let mut buffer = Pixels::new(SIZE as u32, (SIZE / 2.0) as u32, surface).unwrap();
    buffer.set_clear_color(Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    });

    // let mut previous_mouse_position = (0.0, 0.0);

    let mut keyboard_key_states = Vec::with_capacity(50);
    let mut mouse_button_states = [false, false];

    let mut frame = Frame::new();

    event_loop.run(move |event, _, control_flow| {
        for change in receiver.try_iter() {
            match change.event_type {
                ButtonPress(rdev::Button::Left) => mouse_button_states[0] = true,
                ButtonPress(rdev::Button::Right) => mouse_button_states[1] = true,
                ButtonRelease(rdev::Button::Left) => mouse_button_states[0] = false,
                ButtonRelease(rdev::Button::Right) => mouse_button_states[1] = false,

                KeyPress(key) => {
                    if !keyboard_key_states.contains(&key) {
                        keyboard_key_states.push(key)
                    }
                }
                KeyRelease(key) => keyboard_key_states.retain(|&k| k != key),

                _ => (),
            }
        }

        if let Event::RedrawRequested(_) = event {
            for (destination, &source) in buffer
                .get_frame()
                .chunks_exact_mut(4)
                .zip(frame.data().iter())
            {
                destination[0] = (source >> 16) as u8;
                destination[1] = (source >>  8) as u8;
                destination[2] = (source >>  0) as u8;
                destination[3] = (source >> 24) as u8;
            }

            frame.draw_keys(&keyboard_key_states);
            frame.draw_mouse(&mouse_button_states);

            if buffer.render().is_err() {
                *control_flow = ControlFlow::Exit;
            }

            window.request_redraw();
        }
    })
}
