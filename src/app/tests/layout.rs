use super::*;
use crate::config::{Endpoint, Target};
use egui::{Pos2, Rect, Theme, Vec2, epaint::Shape};

#[derive(Clone, Copy, Debug)]
enum Screen {
    Main,
    AddTarget,
    Settings,
}

fn text_geometry(app: &mut App, ctx: &egui::Context, size: Vec2) -> Vec<(Pos2, Vec2)> {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        time: Some(0.0),
        ..Default::default()
    };
    // Measure the grid, size its content-grown area, then center that measured area.
    let _initial_frame = ctx.run(input.clone(), |ctx| app.ui(ctx));
    let _measured_area_frame = ctx.run(input.clone(), |ctx| app.ui(ctx));
    let output = ctx.run(input, |ctx| app.ui(ctx));
    let circles: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            Shape::Circle(circle) if (circle.radius - 19.0).abs() < 0.01 => Some(circle.center),
            _ => None,
        })
        .collect();
    assert_eq!(circles.len(), 1, "one shared refresh indicator");
    for center in circles {
        assert!(
            center.distance(egui::pos2(size.x - 46.0, size.y - 30.0)) < 0.01,
            "countdown must stay bottom-right at {size:?}, found {center:?}"
        );
    }
    let panel_fills: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            Shape::Rect(rect)
                if rect.rect.left() == 0.0
                    && rect.rect.width() == size.x
                    && rect.rect.height() > 1.0
                    && rect.fill.a() == 255 =>
            {
                Some(rect.fill)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        panel_fills,
        vec![
            ctx.style().visuals.panel_fill;
            if app.dialog.is_none() && app.message.is_some() {
                4
            } else {
                3
            }
        ],
        "header, countdown footer, body and any notification must paint the current theme background"
    );
    output
        .shapes
        .into_iter()
        .filter_map(|shape| match shape.shape {
            Shape::Text(text) => Some((text.pos, text.galley.size())),
            _ => None,
        })
        .collect()
}

#[test]
fn switching_themes_preserves_the_actual_screen_layout() {
    for initial_theme in [Theme::Dark, Theme::Light] {
        for width in [700.0, 1100.0, 1440.0] {
            for height in [520.0, 720.0] {
                for populated in [false, true] {
                    for screen in [Screen::Main, Screen::AddTarget, Screen::Settings] {
                        for notification in [false, true] {
                            let config = Config {
                                theme: Some(crate::config::Appearance::Dark),
                                targets: if populated {
                                    vec![
                                        Target {
                                            name: "ICMP".into(),
                                            endpoint: Endpoint::Icmp {
                                                host: "127.0.0.1".into(),
                                            },
                                        },
                                        Target {
                                            name: "TCP".into(),
                                            endpoint: Endpoint::Tcp {
                                                host: "localhost".into(),
                                                port: 443,
                                            },
                                        },
                                        Target {
                                            name: "HTTP".into(),
                                            endpoint: Endpoint::Http {
                                                url: "https://example.com/health".into(),
                                            },
                                        },
                                    ]
                                } else {
                                    Vec::new()
                                },
                                ..Config::default()
                            };
                            let dialog = match screen {
                                Screen::Main => None,
                                Screen::AddTarget => Some(Dialog::AddTarget(Entry::default())),
                                Screen::Settings => Some(Dialog::Settings(config.clone())),
                            };
                            let mut app = App {
                                config,
                                path: Some(PathBuf::from("/example/ping.yml")),
                                monitor: None,
                                message: notification.then(|| "Test notification".into()),
                                writable: true,
                                dialog,
                                beeper: Beeper::default(),
                            };
                            let ctx = egui::Context::default();
                            ctx.set_theme(initial_theme);
                            theme::setup(&ctx);
                            let size = egui::vec2(width, height);
                            ctx.set_theme(Theme::Dark);
                            let dark = text_geometry(&mut app, &ctx, size);
                            let dark_style = ctx.style();
                            ctx.set_theme(Theme::Light);
                            let light = text_geometry(&mut app, &ctx, size);
                            let light_style = ctx.style();

                            assert!(!dark.is_empty(), "the screen did not render");
                            assert_eq!(
                                dark, light,
                                "layout changed at {width}x{height}, initial {initial_theme:?}, populated {populated}, screen {screen:?}, notification {notification}"
                            );
                            assert_ne!(
                                dark_style.visuals.panel_fill,
                                light_style.visuals.panel_fill
                            );
                            let mut normalized_light = (*light_style).clone();
                            normalized_light.visuals = dark_style.visuals.clone();
                            assert_eq!(*dark_style, normalized_light);
                            ctx.set_theme(Theme::Dark);
                            assert_eq!(
                                dark,
                                text_geometry(&mut app, &ctx, size),
                                "switching back changed the layout"
                            );
                        }
                    }
                }
            }
        }
    }
}
