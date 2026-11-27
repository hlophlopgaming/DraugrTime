use super::localization::*;
#[test]
fn supported_locale_detection_overrides_and_fallback() {
	for (locale, expected) in [
		("de_DE.UTF-8", Language::De),
		("fr-CA", Language::Fr),
		("es_MX", Language::Es),
		("pt-BR", Language::Pt),
		("zh-Hans-CN", Language::Zh),
		("ja_JP", Language::Ja),
		("ru_RU", Language::Ru),
		("it_IT", Language::En),
		("zh-TW", Language::En),
	] {
		assert_eq!(Language::detect(locale), expected);
	}
	assert_eq!(resolve(Some(Language::Ja), "de_DE"), Language::Ja);
	assert_eq!(resolve(None, "de_DE"), Language::De);
	assert_eq!(
		translate(Language::Ja, "missing translation"),
		"missing translation"
	);
}
#[test]
fn preferences_persist_and_return_to_system() {
	let path = std::env::temp_dir().join(format!("draugr-language-{}.json", std::process::id()));
	save(&path, Some(Language::Fr)).unwrap();
	assert_eq!(load(&path).unwrap(), Some(Language::Fr));
	save(&path, None).unwrap();
	assert_eq!(load(&path).unwrap(), None);
	std::fs::remove_file(path).unwrap();
}
#[test]
fn catalogs_cover_all_keys_and_preserve_template_parameters() {
	let catalogs = catalogs();
	for language in Language::ALL {
		let catalog = &catalogs[language.code()];
		for (key, value) in catalogs["en"].as_object().unwrap() {
			assert!(
				catalog[key].as_str().is_some_and(|s| !s.trim().is_empty()),
				"{}: {key}",
				language.code()
			);
			assert_eq!(
				parameters(value.as_str().unwrap()),
				parameters(catalog[key].as_str().unwrap()),
				"{}: {key}",
				language.code()
			);
		}
	}
}
fn parameters(s: &str) -> Vec<String> {
	let mut result = Vec::new();
	for part in s.split('{').skip(1) {
		result.push(part.split('}').next().unwrap().to_string());
	}
	result.sort();
	result
}

#[test]
fn duration_units_handle_zero_one_many_and_hours_in_every_language() {
	for language in Language::ALL {
		for (ms, number, unit) in [
			(0, "0", "s"),
			(1000, "1", "s"),
			(2000, "2", "s"),
			(60_000, "1", "m"),
			(120_000, "2", "m"),
		] {
			assert_eq!(
				duration_in(language, ms),
				format!("{number}{}", translate(language, unit))
			);
		}
		assert_eq!(
			duration_in(language, 3_660_000),
			format!(
				"1{} 1{}",
				translate(language, "h"),
				translate(language, "m")
			)
		);
	}
}

#[test]
#[ignore = "changes process language; run alone with --test-threads=1"]
fn native_widgets_render_every_language_screen_and_dialog() {
	use super::*;
	use iced::advanced::{layout, renderer, widget::Tree, Layout, Renderer as _};
	let (mut state, _) = State::boot();
	let shared = draugr_time_daemon::Shared::new(
		draugr_time_core::Storage::open_in_memory().unwrap(),
		"test".into(),
		300,
	);
	shared
		.storage
		.lock()
		.unwrap()
		.tag_interval(
			"Client, Project",
			"",
			state.day.start,
			state.day.start + 60_000,
			false,
		)
		.unwrap();
	state.tags = shared.storage.lock().unwrap().tags().unwrap();
	state.runtime = Some(Runtime::for_test(shared));
	state.all = model::fixture(state.day);
	state.rebuild();
	state.fixture = true;
	state.edit_rule(rules::Edit::New);
	state.edit_rule(rules::Edit::Tag(2));
	let original = current();
	for language in Language::ALL {
		set(language);
		state.cache.clear();
		state.grid_cache.clear();
		for dark in [false, true] {
			state.theme = native_theme(dark);
			for screen in [
				"Day", "Overview", "Week", "Reports", "Tags", "Rules", "Settings",
			] {
				state.screen = screen;
				if screen == "Week" {
					let _ = state.edit_report(reports::Edit::Refresh);
				}
				if screen == "Reports" {
					let _ = state.edit_report(reports::Edit::Preset(0));
				}
				let size = iced::Size::new(900.0, 600.0);
				let viewport = iced::Rectangle::with_size(size);
				let mut renderer = iced::Renderer::new(iced::Font::DEFAULT, 16.0.into());
				let mut view = state.view(window::Id::unique());
				let mut tree = Tree::new(view.as_widget());
				let node = view.as_widget_mut().layout(
					&mut tree,
					&renderer,
					&layout::Limits::new(iced::Size::ZERO, size),
				);
				assert_eq!(node.size(), size, "{} {screen}", language.code());
				fn check_width(layout: Layout<'_>, width: f32, context: &str) {
					assert!(
						layout.bounds().x + layout.bounds().width <= width + 1.0,
						"{context}: {:?}",
						layout.bounds()
					);
					for child in layout.children() {
						check_width(child, width, context);
					}
				}
				check_width(
					Layout::new(&node),
					size.width,
					&format!("{} {screen}", language.code()),
				);
				renderer.reset(viewport);
				view.as_widget().draw(
					&tree,
					&mut renderer,
					&native_theme(dark),
					&renderer::Style {
						text_color: native_theme(dark).palette().text,
					},
					Layout::new(&node),
					iced::mouse::Cursor::Unavailable,
					&viewport,
				);
				if screen == "Week" && language == Language::En {
					if let Some(directory) = std::env::var_os("DRAUGR_TIME_SCREENSHOT_DIR") {
						use iced::advanced::renderer::Headless as _;
						let pixels = renderer.screenshot(
							iced::Size::new(900, 600),
							1.0,
							native_theme(dark).palette().background,
						);
						::image::save_buffer(
							std::path::PathBuf::from(directory).join(if dark {
								"week-dark.png"
							} else {
								"week-light.png"
							}),
							&pixels,
							900,
							600,
							::image::ColorType::Rgba8,
						)
						.unwrap();
					}
				}
			}
			state.screen = "Tags";
			let _ = state.edit(tagging::Edit::Tag(2));
			let renderer = iced::Renderer::new(iced::Font::DEFAULT, 16.0.into());
			let mut dialog = state.tag_editor();
			let mut tree = Tree::new(dialog.as_widget());
			let node = dialog.as_widget_mut().layout(
				&mut tree,
				&renderer,
				&layout::Limits::new(iced::Size::ZERO, iced::Size::new(626.0, 500.0)),
			);
			assert!(node.size().width <= 626.0);
			drop(dialog);
			state.editor = None;
		}
	}
	set(original);
}
