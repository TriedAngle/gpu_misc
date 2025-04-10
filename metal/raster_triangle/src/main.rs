use cocoa::appkit::{NSView, NSWindow};
use cocoa::base::id as cocoa_id;
use metal::*;
use objc::rc::autoreleasepool;
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::{Window, WindowId},
};

// Define vertex struct and buffer indices
#[repr(C)]
#[derive(Clone, Copy)]
struct AAPLVertex {
    position: [f32; 2], // 2D position
    color: [f32; 4],    // RGBA color
}

const AAPL_VERTEX_INPUT_INDEX_VERTICES: u64 = 0;
const AAPL_VERTEX_INPUT_INDEX_VIEWPORT_SIZE: u64 = 1; // Index for viewport size buffer

// MetalState manages Metal resources and rendering
struct MetalState {
    window: Arc<Window>,
    device: Device,
    layer: MetalLayer,
    command_queue: CommandQueue,
    pipeline_state: RenderPipelineState,
    vertex_buffer: Buffer,
    viewport_buffer: Buffer,
}

impl MetalState {
    fn new(window: Arc<Window>) -> Self {
        let device = Device::system_default().expect("No Metal device found");

        let mut layer = MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        layer.set_presents_with_transaction(false);
        unsafe {
            if let Ok(RawWindowHandle::AppKit(rw)) = window.window_handle().map(|wh| wh.as_raw()) {
                let view = rw.ns_view.as_ptr() as cocoa_id;
                view.setWantsLayer(true);
                view.setLayer(<*mut _>::cast(layer.as_mut()));
            }
        }

        let command_queue = device.new_command_queue();

        let library = device
            .new_library_with_source(include_str!("shaders.metal"), &CompileOptions::new())
            .expect("Failed to create shader library");

        let vertex_function = library
            .get_function("vertexShader", None)
            .expect("Failed to find vertex function");
        let fragment_function = library
            .get_function("fragmentShader", None)
            .expect("Failed to find fragment function");

        let pipeline_state_descriptor = RenderPipelineDescriptor::new();
        pipeline_state_descriptor.set_label("Simple Pipeline");
        pipeline_state_descriptor.set_vertex_function(Some(&vertex_function));
        pipeline_state_descriptor.set_fragment_function(Some(&fragment_function));
        let color_attachment = pipeline_state_descriptor
            .color_attachments()
            .object_at(0)
            .unwrap();
        color_attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);

        let vertex_descriptor = VertexDescriptor::new();

        let position_attribute = vertex_descriptor.attributes().object_at(0).unwrap();
        position_attribute.set_format(MTLVertexFormat::Float2);
        position_attribute.set_offset(0);
        position_attribute.set_buffer_index(AAPL_VERTEX_INPUT_INDEX_VERTICES);

        let color_attribute = vertex_descriptor.attributes().object_at(1).unwrap();
        color_attribute.set_format(MTLVertexFormat::Float4);
        color_attribute.set_offset(8);
        color_attribute.set_buffer_index(AAPL_VERTEX_INPUT_INDEX_VERTICES);

        let layout = vertex_descriptor
            .layouts()
            .object_at(AAPL_VERTEX_INPUT_INDEX_VERTICES)
            .unwrap();
        layout.set_stride(size_of::<AAPLVertex>() as u64);
        layout.set_step_rate(1);
        layout.set_step_function(MTLVertexStepFunction::PerVertex);
        pipeline_state_descriptor.set_vertex_descriptor(Some(&vertex_descriptor));

        let pipeline_state = device
            .new_render_pipeline_state(&pipeline_state_descriptor)
            .expect("Failed to create pipeline state");

        let triangle_vertices = [
            AAPLVertex {
                position: [250.0, -250.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            AAPLVertex {
                position: [-250.0, -250.0],
                color: [0.0, 1.0, 0.0, 1.0],
            },
            AAPLVertex {
                position: [0.0, 250.0],
                color: [0.0, 0.0, 1.0, 1.0],
            },
        ];

        let vertex_buffer = device.new_buffer_with_data(
            triangle_vertices.as_ptr() as *const c_void,
            (size_of::<AAPLVertex>() * triangle_vertices.len()) as u64,
            MTLResourceOptions::StorageModeShared,
        );

        let viewport_buffer = device.new_buffer(
            size_of::<[f32; 2]>() as u64,
            MTLResourceOptions::StorageModeShared,
        );

        MetalState {
            window,
            device,
            layer,
            command_queue,
            pipeline_state,
            vertex_buffer,
            viewport_buffer,
        }
    }

    fn update_viewport_buffer(&self, view_size: [f32; 2]) {
        let contents = self.viewport_buffer.contents();
        unsafe {
            std::ptr::copy_nonoverlapping(
                view_size.as_ptr(),
                contents as *mut f32,
                view_size.len(),
            );
        }
    }

    fn render(&self) {
        if let Some(drawable) = self.layer.next_drawable() {
            autoreleasepool(|| {
                let view_size = [
                    self.layer.drawable_size().width as f32,
                    self.layer.drawable_size().height as f32,
                ];

                self.update_viewport_buffer(view_size);

                let render_pass_descriptor = RenderPassDescriptor::new();
                let color_attachment = render_pass_descriptor
                    .color_attachments()
                    .object_at(0)
                    .unwrap();
                color_attachment.set_texture(Some(drawable.texture()));
                color_attachment.set_load_action(MTLLoadAction::Clear);
                color_attachment.set_clear_color(MTLClearColor::new(0.0, 0.5, 0.7, 1.0)); // Cyan background
                color_attachment.set_store_action(MTLStoreAction::Store);

                let command_buffer = self.command_queue.new_command_buffer();
                let render_encoder =
                    command_buffer.new_render_command_encoder(&render_pass_descriptor);

                let viewport = MTLViewport {
                    originX: 0.0,
                    originY: 0.0,
                    width: view_size[0] as f64,
                    height: view_size[1] as f64,
                    znear: 0.0,
                    zfar: 1.0,
                };
                render_encoder.set_viewport(viewport);

                render_encoder.set_render_pipeline_state(&self.pipeline_state);

                render_encoder.set_vertex_buffer(
                    AAPL_VERTEX_INPUT_INDEX_VERTICES,
                    Some(&self.vertex_buffer),
                    0,
                );

                render_encoder.set_vertex_buffer(
                    AAPL_VERTEX_INPUT_INDEX_VIEWPORT_SIZE,
                    Some(&self.viewport_buffer),
                    0,
                );

                render_encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
                render_encoder.end_encoding();

                command_buffer.present_drawable(&drawable);
                command_buffer.commit();
            });
        }
    }
}

struct App {
    window: Option<Arc<Window>>,
    metal_state: Option<MetalState>,
}

impl Default for App {
    fn default() -> Self {
        App {
            window: None,
            metal_state: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Metal Triangle with Buffers")
                        .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0)),
                )
                .unwrap(),
        );

        self.metal_state = Some(MetalState::new(window.clone()));
        self.metal_state.as_ref().unwrap().window.request_redraw();
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(metal_state) = &self.metal_state {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(KeyCode::Escape),
                            ..
                        },
                    ..
                } => event_loop.exit(),
                WindowEvent::RedrawRequested => {
                    metal_state.render();
                    metal_state.window.request_redraw();
                }
                _ => (),
            }
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Failed to run app");
}
