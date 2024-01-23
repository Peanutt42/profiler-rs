use eframe::egui;
use std::path::Path;
use profiler::Profiler;

mod processed_profiler;
use processed_profiler::ProcessedProfiler;

fn main() -> eframe::Result<()>{
	let options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
		..Default::default()
	};

	let mut show_open_file_dialog = true;
	let mut loading_error_msg: Option<String> = None;
	let mut offset: f32 = 0.0;
	let mut zoom: f32 = 1.0;

	let mut screen_width = 800.0;
	let mut mouse_pos = egui::Pos2::new(0.0, 0.0);

	let mut profiler: Option<ProcessedProfiler> = None;

	eframe::run_simple_native("Profiler GUI", options, move |ctx, _frame| {
		// drag and drop
		ctx.input(|i| {
			for file in i.raw.dropped_files.iter() {
				let mut loaded_profiler = Profiler::new();
				if let Err(e) = loaded_profiler.load_from_file(Path::new(&file.path.clone().unwrap())) {
					loading_error_msg = Some(e.to_string());
					show_open_file_dialog = true;
				}
				else {
					loading_error_msg = None;
					profiler = Some(ProcessedProfiler::new(&loaded_profiler));
					show_open_file_dialog = false;
				}
			}
		});

		// timeline input
		ctx.input(|i| {
			if let Some(pos) = i.pointer.latest_pos() {
				mouse_pos = pos;
			}

			for e in i.events.iter() {
				match e {
					egui::Event::MouseWheel { unit: _, delta, modifiers: _ } => {
						let factor = delta.y * 0.15 + 1.0;
						zoom *= factor;
						offset -= (mouse_pos.x - (screen_width / 2.0)) / zoom * ((1.0 / factor) - 1.0);
					}
					_ => {},
				}
			}

			if i.pointer.primary_down() {
				offset -= i.pointer.delta().x / zoom;
			}
			if i.pointer.secondary_down() {
				// just zooms at the center
				zoom *= i.pointer.delta().y * 0.005 + 1.0;
			}
		});

		egui::CentralPanel::default().show(ctx, |ui| {
			if profiler.is_none() {
				return;
			}

			if let Some(profiler) = &profiler {
				let canvas = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("profile_results")));
				
				let mut total_time = 0.0;
				if !profiler.frames.is_empty() {
					for profile_result in profiler.frames.last().unwrap().profile_results.iter() {
						let end_time = (profile_result.start + profile_result.duration).as_secs_f32();
						if total_time < end_time {
							total_time = end_time;
						}
					}
				}

				screen_width = ctx.screen_rect().width();
				let center_x = screen_width / 2.0;
				let height = 20.0;

				for frame in profiler.frames.iter() {
					let frame_start_pixel = center_x + (frame.start.as_secs_f32() * screen_width / total_time - offset) * zoom;
					let frame_end_pixel = center_x + ((frame.start + frame.duration).as_secs_f32() * screen_width / total_time - offset) * zoom;
					if frame_start_pixel > screen_width || frame_end_pixel < 0.0 {
						continue;
					}
					
					for profile_result in frame.profile_results.iter() {
						let x = center_x + (profile_result.start.as_secs_f32() * screen_width / total_time - offset) * zoom;
						let y = profile_result.depth as f32 * height;
						let width = (profile_result.duration.as_secs_f32() / total_time) * screen_width * zoom;
						
						let rect = egui::Rect::from_min_size(egui::Pos2::new(x, y), egui::Vec2::new(width, height));
						canvas.rect(rect, 2.5, egui::Color32::BLUE, egui::Stroke::new(1.5, egui::Color32::BLACK));

						let mut allow_tooltip = false;
						if width > 50.0 {
							let truncated = draw_truncated_text(&canvas, ui, &profile_result.name, width, rect.center());
							if truncated {
                                allow_tooltip = true;
                            }
						}
						else {
							allow_tooltip = true;
						}
						if allow_tooltip && mouse_pos.x >= x && mouse_pos.y >= y && mouse_pos.y <= y + height && mouse_pos.x <= x + width {
							egui::show_tooltip_at_pointer(ctx, egui::Id::new("profiler_result_tooltip"), |ui| {
								ui.label(&profile_result.name);
							});
						}
					}
				}
			}
		});

		if show_open_file_dialog {
			egui::Window::new("Open saved profiling record")
				.default_size([300.0, 150.0])
				.collapsible(false)
				.movable(false)
				.anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
				.show(ctx, |ui|
			{
				ui.label("Drag and drop a file or ...");

				if ui.button("Load").clicked() {
					if let Some(filepath) =  rfd::FileDialog::new().add_filter("YAML", &["yaml", "yml"]).pick_file() {
						let mut loaded_profiler = Profiler::new();
						if let Err(e) = loaded_profiler.load_from_file(&Path::new(&filepath)) {
							loading_error_msg = Some(e);
						}
						else {
							loading_error_msg = None;
							profiler = Some(ProcessedProfiler::new(&loaded_profiler));
							show_open_file_dialog = false;
						}
					}
				}

				if let Some(error) = loading_error_msg.clone() {
					ui.visuals_mut().override_text_color = Some(egui::Color32::RED);

					ui.label(&error);
				}
			});
		}
	})
}


fn glyph_width(text: String, font_id: egui::FontId, canvas: &egui::Painter) -> f32 {
	canvas.layout_no_wrap(text, font_id, egui::Color32::WHITE).rect.width()
}
fn glyph_char_width(c: char, font_id: egui::FontId, ui: &mut egui::Ui) -> f32 {
	ui.fonts(|f| f.glyph_width(&font_id, c))
}

// returns wheter the text was truncated
fn draw_truncated_text(painter: &egui::Painter, ui: &mut egui::Ui, text: &str, max_width: f32, pos: egui::Pos2) -> bool {
	let font_id = egui::TextStyle::Body.resolve(ui.style());

    let text_width = glyph_width(text.to_string(), font_id.clone(), painter);
	
	let truncated_text = if text_width > max_width {
        let ellipsis_width = glyph_width("...".to_string(), font_id.clone(), painter);
        let mut current_width = 0.0;
        let mut truncated_length = 0;
        for (i, char_width) in text.chars().map(|c| glyph_char_width(c, font_id.clone(), ui)).enumerate() {
            current_width += char_width;
            if current_width + ellipsis_width > max_width {
                break;
            }
            truncated_length = i + 1;
        }

        let mut truncated_text = String::with_capacity(truncated_length + 3);
        truncated_text.push_str(&text[..truncated_length]);
        truncated_text.push_str("...");
        truncated_text
    } else {
        text.to_owned()
    };

    painter.text(pos, egui::Align2::CENTER_CENTER, truncated_text, font_id, egui::Color32::WHITE);

	text_width > max_width
}