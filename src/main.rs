#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "eframe template",
        native_options,
        Box::new(|cc| Ok(Box::new(eframe_template::ModelViewerApp::new(cc)))),
    )
}

pub async fn sleep(delay: i32) {
    let mut cb = |resolve: web_sys::js_sys::Function, reject: web_sys::js_sys::Function| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay);
    };

    let p = web_sys::js_sys::Promise::new(&mut cb);

    wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use std::sync::Arc;

    use eframe::wasm_bindgen::JsCast as _;
    use rs_model_viewer::runetek5::{
        graphics::texture::TextureProvider,
        js5::{
            net::{Openrs2Js5NetClient, Openrs2Js5ResourceProvider},
            Js5, Js5ResourceProvider,
        },
    };

    // Redirect `log` message to `console.log` and friends:
    // let is_release = cfg!(debug_assertions);
    // eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let mut web_options = eframe::WebOptions::default();
    web_options.depth_buffer = 24;

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let net_client = Arc::new(Openrs2Js5NetClient::new(2064));

        let resource_provider = Arc::new(Openrs2Js5ResourceProvider::new(7, net_client.clone()));
        let model_js5 = loop {
            let index = resource_provider.fetch_index();
            if let Some(index) = index {
                break Arc::new(Js5::new(resource_provider.clone(), index, false, false));
            }
            sleep(20).await;
        };

        let resource_provider = Arc::new(Openrs2Js5ResourceProvider::new(8, net_client.clone()));
        let sprite_js5 = loop {
            let index = resource_provider.fetch_index();
            if let Some(index) = index {
                break Arc::new(Js5::new(resource_provider.clone(), index, false, false));
            }
            sleep(20).await;
        };
        let resource_provider = Arc::new(Openrs2Js5ResourceProvider::new(9, net_client.clone()));
        let texture_js5 = loop {
            let index = resource_provider.fetch_index();
            if let Some(index) = index {
                break Arc::new(Js5::new(resource_provider.clone(), index, false, false));
            }
            sleep(20).await;
        };

        loop {
            if texture_js5.fetch_all() {
                break;
            }
            sleep(20).await;
        }

        let texture_provider = TextureProvider::new(sprite_js5.clone(), &texture_js5);

        loop {
            let loaded_percentage = texture_provider.get_loaded_percentage();
            if loaded_percentage == 100 {
                break;
            }
            println!("Loaded: {}%", loaded_percentage);
            sleep(20).await;
        }

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| {
                    Ok(Box::new(rs_model_viewer::ModelViewerApp::new(
                        cc,
                        model_js5,
                        texture_provider,
                    )))
                }),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
