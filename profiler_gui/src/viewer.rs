use eframe::egui;
use profiler::GlobalProfiler;
use std::{path::Path, time::Duration, collections::HashMap};
use crate::ProcessedGlobalProfiler;
use crate::utils::draw_truncated_text;

pub struct Viewer {
	show_open_file_dialog: bool,
	loading_error_msg: Option<String>,
	view_start: f64,
	view_end: f64,
	screen_width: f64,
	screen_height: f64,
	mouse_pos: egui::Pos2,
	profiler: Option<ProcessedGlobalProfiler>,
	thread_profilers_collapsed: HashMap<String, bool>,
}

impl Viewer {
	pub fn new() -> Self {
		Viewer {
            show_open_file_dialog: true,
            loading_error_msg: None,
            view_start: 0.0,
            view_end: 1.0,
            screen_width: 800.0,
            screen_height: 600.0,
            mouse_pos: egui::Pos2::new(0.0, 0.0),
            profiler: None,
			thread_profilers_collapsed: HashMap::new(),
        }
	}

	fn calc_pos_x(&self, x: f64) -> f64 {
		if let Some(profiler) = &self.profiler {
			let relative_pos = x / profiler.total_time.as_secs_f64();
			(self.view_start * (1.0 - relative_pos) + self.view_end * relative_pos) * self.screen_width
		}
		else {
			0.0
		}
	}

	pub fn update(&mut self, ctx: &egui::Context) {
		// disable reactive mode
		ctx.request_repaint();

		self.handle_drag_and_drop(ctx);

		self.handle_input(ctx);

		egui::CentralPanel::default().show(ctx, |ui| {
			if self.profiler.is_none() {
				return;
			}

			self.screen_width = ctx.screen_rect().width() as f64;
			self.screen_height = ctx.screen_rect().height() as f64;
			let rounding = 2.5;
			let hover_rect_offset = 1.0;
			
			egui::Frame::dark_canvas(ui.style()).show(ui, |ui| {
				let available_height = ui.max_rect().bottom() - ui.min_rect().bottom();

				egui::ScrollArea::vertical().show(ui, |ui| {
					let mut canvas = ui.available_rect_before_wrap();
					canvas.max.y = f32::INFINITY;
					let response = ui.interact(canvas, ui.id(), egui::Sense::click_and_drag());
					self.handle_interaction(ctx, &response);

					let mut cursor_y = canvas.top() as f64;

					let profiler = self.profiler.as_ref().unwrap();

					let mut selection_rect = None;
					for thread_profiler in profiler.thread_profilers.values() {
						let mut collapsed = self.thread_profilers_collapsed.get(&thread_profiler.name).copied().unwrap_or(false);
						self.draw_thread_profiler(ctx, ui, thread_profiler, &mut selection_rect, &mut cursor_y, &mut collapsed, canvas, rounding, hover_rect_offset);
						self.thread_profilers_collapsed.insert(thread_profiler.name.clone(), collapsed);
					}
					if let Some(selection_rect) = selection_rect {
						ui.painter().rect_stroke(selection_rect, rounding, egui::Stroke::new(2.0 * hover_rect_offset, egui::Color32::YELLOW));
					}

					let mut used_rect = canvas;
					used_rect.max.y = cursor_y as f32;
					used_rect.max.y = used_rect.max.y.max(used_rect.min.y + available_height);
					ui.allocate_rect(used_rect, egui::Sense::hover());
				});
			});
		});		
		
		if self.show_open_file_dialog {
			self.open_file_dialog(ctx);
		}
	}

	#[allow(clippy::too_many_arguments)]
	fn draw_thread_profiler(&self, ctx: &egui::Context, ui: &mut egui::Ui, thread_profiler: &profiler::ThreadProfiler, selecton_rect: &mut Option<egui::Rect>, cursor_y: &mut f64, collapsed: &mut bool, canvas: egui::Rect, rounding: f32, hover_rect_offset: f32) {
		let function_height = 28.0;
		let text_height = 15.0;
		let seperator_height = 1.0;

		ui.painter().with_clip_rect(canvas).rect_filled(egui::Rect::from_min_size(egui::pos2(0.0, *cursor_y as f32), egui::vec2(self.screen_width as f32, seperator_height as f32)), 0.0, egui::Color32::WHITE);
		*cursor_y += seperator_height;
		let thread_name_height = *cursor_y;
		*cursor_y += text_height;
		let mut largest_frame_height = 0.0;

		if !*collapsed {
			for frame in thread_profiler.frames.iter() {
				let frame_start_pixel = self.calc_pos_x(frame.start.as_secs_f64());
				let frame_end_pixel = self.calc_pos_x((frame.start + frame.duration).as_secs_f64());
				if frame_start_pixel > self.screen_width || frame_end_pixel < 0.0 {
					continue;
				}
				
				for scope_result in frame.scope_results.iter() {
					let x = self.calc_pos_x(scope_result.start.as_secs_f64());
					let y = scope_result.depth as f64 * function_height + *cursor_y;
					let width = self.calc_pos_x((scope_result.start + scope_result.duration).as_secs_f64()) - x;
					
					if (scope_result.depth + 1) as f64 * function_height > largest_frame_height {
						largest_frame_height = (scope_result.depth + 1) as f64 * function_height;
					}
					
					if x > self.screen_width || x + width < 0.0 || y > self.screen_height {
						continue;
					}
					
					let rect = egui::Rect::from_min_size(egui::Pos2::new(x as f32, y as f32), egui::Vec2::new(width as f32, function_height as f32));
					let painter = ui.painter().with_clip_rect(rect.intersect(canvas));
					if width > 10.0 {
						painter.rect(rect, rounding, egui::Color32::BLUE, egui::Stroke::new(1.5, egui::Color32::BLACK));
						draw_truncated_text(ui, &scope_result.name, width as f32, rect.center(), rect.intersect(canvas));
					}
					else {
						painter.rect_filled(rect, 0.0, egui::Color32::BLUE);
					}
					
					let hovered: bool = self.mouse_pos.x as f64 >= x && self.mouse_pos.y as f64 >= y && self.mouse_pos.y as f64 <= y + function_height && self.mouse_pos.x as f64 <= x + width;
					if hovered {
						*selecton_rect = Some(egui::Rect::from_min_size(rect.min - egui::Vec2::new(hover_rect_offset, hover_rect_offset), rect.size() + egui::Vec2::new(2.0 * hover_rect_offset, 2.0 * hover_rect_offset)));
						
						self.draw_tooltip(ctx, scope_result, frame, &thread_profiler.name);
					}
				}
			}
		}
		let text = format!("{} {}", if *collapsed { "⏵" } else { "⏷" }, thread_profiler.name);
		let thread_name_rect = ui.painter().text(egui::pos2(canvas.min.x, thread_name_height as f32), egui::Align2::LEFT_TOP, text, egui::FontId::default(), egui::Color32::WHITE);
		let thread_name_response = ui.interact(thread_name_rect, egui::Id::new(format!("thread_name_{}", thread_profiler.name)), egui::Sense::click());
		if thread_name_response.clicked() {
			*collapsed = !*collapsed;
		}
		*cursor_y += text_height;

		*cursor_y += largest_frame_height;
	}

	fn draw_tooltip(&self, ctx: &egui::Context, scope_result: &profiler::ScopeResult, frame: &profiler::Frame, thread_name: &String) {
		egui::show_tooltip_at_pointer(ctx, egui::Id::new("profiler_result_tooltip"), |ui| {
			ui.label(&scope_result.name);
			ui.label(format!("Duration: {}", format_duration(&scope_result.duration)));

			let mut self_duration = scope_result.duration;
			for i in 0..frame.scope_results.len() {
				if scope_result.depth + 1 == frame.scope_results[i].depth && frame.scope_results[i].is_inside(scope_result) {
					self_duration -= frame.scope_results[i].duration;
				}
			}
			ui.label(format!("Self Duration: {}", format_duration(&self_duration)));
			ui.label(format!("Thread: {}", thread_name));
		});
	}

	fn handle_interaction(&mut self, ctx: &egui::Context, response: &egui::Response) {
		if response.dragged_by(egui::PointerButton::Primary) {
			self.view_start += response.drag_delta().x as f64 / self.screen_width;
			self.view_end += response.drag_delta().x as f64 / self.screen_width;
			ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
		}

		// 0.0..1.0 inside the current view
		let mouse_pos_relative_to_view = ((self.mouse_pos.x as f64 / self.screen_width) - self.view_start) / (self.view_end - self.view_start);
		let zoom_target = self.view_start + (self.view_end - self.view_start) * mouse_pos_relative_to_view;
		
		if response.dragged_by(egui::PointerButton::Secondary) {
			self.zoom(-0.005 * response.drag_delta().y as f64, zoom_target);
			ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
		}
	}

	fn handle_input(&mut self, ctx: &egui::Context) {
		ctx.input(|i| {
			// keyboard
			if i.key_down(egui::Key::A) || i.key_down(egui::Key::ArrowLeft) {
				self.view_start += 1.0 * i.unstable_dt as f64;
				self.view_end += 1.0 * i.unstable_dt as f64;
			}
			if i.key_down(egui::Key::D) || i.key_down(egui::Key::ArrowRight) {
				self.view_start -= 1.0 * i.unstable_dt as f64;
				self.view_end -= 1.0 * i.unstable_dt as f64;
			}
			if i.key_down(egui::Key::W) {
				self.zoom(-0.02, 0.5);
			}
			if i.key_down(egui::Key::S) {
				self.zoom(0.02, 0.5);
			}
			
			// mouse
			if let Some(pos) = i.pointer.latest_pos() {
				self.mouse_pos = pos;
			}
		});

		if self.view_start >= self.view_end {
			self.view_start = 0.0;
			self.view_end  = 1.0;
		}
	}

	fn zoom(&mut self, mut amount: f64, zoom_target: f64) {
		if amount > 0.9 {
			amount = 0.9;
		}
		self.view_start += (zoom_target - self.view_start) * amount;
		self.view_end -= (self.view_end - zoom_target) * amount;
	}

	fn load_profiler(&mut self, filepath: &Path) {
		let mut loaded_profiler = GlobalProfiler::new();
		if let Err(e) = loaded_profiler.load_from_file(filepath) {
			self.loading_error_msg = Some(e.to_string());
			self.show_open_file_dialog = true;
		}
		else {
			self.loading_error_msg = None;
			let global_profiler = ProcessedGlobalProfiler::new(loaded_profiler.thread_profilers);
			for thread_profiler in global_profiler.thread_profilers.values() {
				self.thread_profilers_collapsed.insert(thread_profiler.name.clone(), false);
			}
			self.profiler = Some(global_profiler);
			self.view_start  = 0.0;
			self.view_end = 1.0;
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
				if let Some(filepath) =  rfd::FileDialog::new().add_filter("Profiling", &["profiling"]).pick_file() {
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
		return format!("{secs_f32} s");
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