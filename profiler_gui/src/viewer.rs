use eframe::egui;
use crate::ProcessedProfiler;
use profiler::Profiler;
use std::{path::Path, time::Duration};

pub struct Viewer {
	show_open_file_dialog: bool,
	loading_error_msg: Option<String>,
	offset: f32,
	zoom: f32,
	screen_width: f32,
	mouse_pos: egui::Pos2,
	profiler: Option<ProcessedProfiler>,
}

impl Viewer {
	pub fn new() -> Self {
		Viewer {
            show_open_file_dialog: true,
            loading_error_msg: None,
            offset: 0.0,
            zoom: 1.0,
            screen_width: 800.0,
            mouse_pos: egui::Pos2::new(0.0, 0.0),
            profiler: None,
        }
	}

	fn calc_pos(&self, x: f32) -> f32 {
		if let Some(profiler) = &self.profiler {
			self.screen_width / 2.0 + (x * self.screen_width / profiler.total_time.as_secs_f32() - self.offset) * self.zoom
		}
		else {
			0.0
		}
	}

	pub fn update(&mut self, ctx: &egui::Context) {
		self.handle_drag_and_drop(ctx);

		self.handle_input(ctx);

		egui::CentralPanel::default().show(ctx, |ui| {
			if self.profiler.is_none() {
				return;
			}
			let profiler = self.profiler.as_ref().unwrap();

			let canvas = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("profile_results")));

			let timeline_height = self.draw_timeline(&canvas, ctx);
			let padding = 10.0;

			self.screen_width = ctx.screen_rect().width();
			let height = 28.0;
			let rounding = 2.5;
			let hover_rect_offset = 1.0;

			let mut selection_rect: Option<egui::Rect> = None;

			for frame in profiler.frames.iter() {
				let frame_start_pixel = self.calc_pos(frame.start.as_secs_f32());
				let frame_end_pixel = self.calc_pos((frame.start + frame.duration).as_secs_f32());
				if frame_start_pixel > self.screen_width || frame_end_pixel < 0.0 {
					continue;
				}
				
				for profile_result in frame.profile_results.iter() {
					let x = self.calc_pos(profile_result.start.as_secs_f32());
					let y = profile_result.depth as f32 * height + timeline_height + padding;
					let width = (profile_result.duration.as_secs_f32() / profiler.total_time.as_secs_f32()) * self.screen_width * self.zoom;
					
					let rect = egui::Rect::from_min_size(egui::Pos2::new(x, y), egui::Vec2::new(width, height));
					canvas.rect(rect, rounding, egui::Color32::BLUE, egui::Stroke::new(1.5, egui::Color32::BLACK));

					let mut show_name_tooltip = false;
					if width > 50.0 {
						let truncated = draw_truncated_text(&canvas, ui, &profile_result.name, width, rect.center());
						if truncated {
							show_name_tooltip = true;
						}
					}
					else {
						show_name_tooltip = true;
					}
					
					let hovered: bool = self.mouse_pos.x >= x && self.mouse_pos.y >= y && self.mouse_pos.y <= y + height && self.mouse_pos.x <= x + width;
					if hovered {
						selection_rect = Some(egui::Rect::from_min_size(rect.min - egui::Vec2::new(hover_rect_offset, hover_rect_offset), rect.size() + egui::Vec2::new(2.0 * hover_rect_offset, 2.0 * hover_rect_offset)));
						
						egui::show_tooltip_at_pointer(ctx, egui::Id::new("profiler_result_tooltip"), |ui| {
							if show_name_tooltip {
								ui.label(&profile_result.name);
							}
							ui.label(format!("Duration: {}", format_duration(&profile_result.duration)));
						});
					}
				}
			}
			if let Some(selection_rect) = selection_rect {
				canvas.rect_stroke(selection_rect, rounding, egui::Stroke::new(2.0 * hover_rect_offset, egui::Color32::YELLOW));
			}
		});

		if self.show_open_file_dialog {
			self.open_file_dialog(ctx);
		}
	}

	// returns the height of the timeline
	fn draw_timeline(&self, canvas: &egui::Painter, ctx: &egui::Context) -> f32 {
		if self.profiler.is_none() || self.profiler.as_ref().unwrap().frames.is_empty() {
			return 0.0;
		}

		let profiler = self.profiler.as_ref().unwrap();
		
		let height = 15.0;
		let screen_width = ctx.screen_rect().width();

		// start      0             width    end
		//  |         #################        |
		let start = self.calc_pos(0.0);
		let end = self.calc_pos(profiler.total_time.as_secs_f32());

		let left_percentage = (0.0 - start) / (end - start);
		let right_percentage = (screen_width - start) / (end - start);
		let view_start = left_percentage * screen_width;
		let view_end = right_percentage * screen_width;

		canvas.rect_filled(egui::Rect::from_min_max(egui::Pos2::new(view_start, 0.0), egui::Pos2::new(view_end, height)), 0.0, egui::Color32::WHITE);

		for frame in profiler.frames.iter() {
			canvas.rect(egui::Rect::from_min_size(egui::Pos2::new((frame.start.as_secs_f32() / profiler.total_time.as_secs_f32()) * screen_width, 0.0), egui::Vec2::new(1.0, height)), 0.0, egui::Color32::GRAY, egui::Stroke::new(1.0, egui::Color32::BLACK));
		}

		height
	}

	fn handle_input(&mut self, ctx: &egui::Context) {
		ctx.input(|i| {
			if let Some(pos) = i.pointer.latest_pos() {
				self.mouse_pos = pos;
			}

			for e in i.events.iter() {
				if let egui::Event::MouseWheel { unit: _, delta, modifiers: _ } = e {
					let factor = delta.y * 0.15 + 1.0;
					self.zoom *= factor;
					self.offset -= (self.mouse_pos.x - (self.screen_width / 2.0)) / self.zoom * ((1.0 / factor) - 1.0);
				}
			}

			if i.pointer.primary_down() {
				self.offset -= i.pointer.delta().x / self.zoom;
			}
			if i.pointer.secondary_down() {
				// just zooms at the center
				self.zoom *= i.pointer.delta().y * 0.005 + 1.0;
			}
		});
	}

	fn load_profiler(&mut self, filepath: &Path) {
		let mut loaded_profiler = Profiler::new();
		if let Err(e) = loaded_profiler.load_from_file(filepath) {
			self.loading_error_msg = Some(e.to_string());
			self.show_open_file_dialog = true;
		}
		else {
			self.loading_error_msg = None;
			self.profiler = Some(ProcessedProfiler::new(&loaded_profiler));
			self.offset = self.screen_width / 2.0;
			self.show_open_file_dialog = false;
		}
	}

	fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
		ctx.input(|i| {
			for file in i.raw.dropped_files.iter() {
				self.load_profiler(file.path.as_ref().unwrap());
			}
		});
	}

	fn open_file_dialog(&mut self, ctx: &egui::Context) {
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
					self.load_profiler(&filepath);
				}
			}

			if let Some(error) = self.loading_error_msg.clone() {
				ui.visuals_mut().override_text_color = Some(egui::Color32::RED);

				ui.label(&error);
			}
		});
	}
}

fn format_duration(duration: &Duration) -> String {
	const NANOS_PER_SEC: f32 = 1_000_000_000.0;
	const NANOS_PER_MILLI: f32 = 1_000_000.0;
	const NANOS_PER_MICRO: f32 = 1_000.0;
	const MILLIS_PER_SEC: f32 = 1_000.0;
	const MICROS_PER_SEC: f32 = 1_000_000.0;

	let secs = duration.as_secs() as f32;
	let nanos = duration.subsec_nanos() as f32;
	let secs_f32 = secs + nanos / NANOS_PER_SEC;
	if secs_f32 >= 0.1 {
		return format!("{secs} s");
	}
	let millis = secs * MILLIS_PER_SEC + nanos / NANOS_PER_MILLI;
	if millis >= 0.1 {
		return format!("{millis} ms");
	}
	let micros = secs * MICROS_PER_SEC + nanos / NANOS_PER_MICRO;
	if micros >= 0.1 {
		return format!("{micros} us");
	}
	let nanos = secs * NANOS_PER_SEC + nanos;
	format!("{nanos} ns")
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
	
	if text_width > max_width {
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
		painter.text(pos, egui::Align2::CENTER_CENTER, truncated_text, font_id, egui::Color32::WHITE);
		true
	} else {
		painter.text(pos, egui::Align2::CENTER_CENTER, text, font_id, egui::Color32::WHITE);
		false
	}
}