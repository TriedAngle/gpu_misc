use std::mem::size_of;

use metal::*;
use objc::rc::autoreleasepool;

fn main() {
    let array_length = 1024;

    autoreleasepool(|| {
        let device = Device::system_default().expect("No Metal device found");
        println!("Using device: {}", device.name());

        let command_queue = device.new_command_queue();

        let buffer_size = (array_length * size_of::<f32>()) as u64;
        
        let buffer_a = device.new_buffer(
            buffer_size,
            MTLResourceOptions::StorageModeShared
        );

        let buffer_b = device.new_buffer(
            buffer_size,
            MTLResourceOptions::StorageModeShared
        );

        let result_buffer = device.new_buffer(
            buffer_size,
            MTLResourceOptions::StorageModeShared
        );

        generate_random_float_data(&buffer_a, array_length);
        generate_random_float_data(&buffer_b, array_length);

        let shader_source = include_str!("add.metal");
        let compile_options = CompileOptions::new();
        let library = device.new_library_with_source(shader_source, &compile_options)
            .expect("Failed to compile Metal shader");
        let add_function = library.get_function("add_arrays", None)
            .expect("Failed to find the add_arrays function");

        let pipeline_state = device
            .new_compute_pipeline_state_with_function(&add_function)
            .expect("Failed to create pipeline state");

        let command_buffer = command_queue.new_command_buffer();

        let compute_encoder = command_buffer.new_compute_command_encoder();

        compute_encoder.set_compute_pipeline_state(&pipeline_state);
        compute_encoder.set_buffer(0, Some(&buffer_a), 0);
        compute_encoder.set_buffer(1, Some(&buffer_b), 0);
        compute_encoder.set_buffer(2, Some(&result_buffer), 0);

        let grid_size = MTLSize {
            width: array_length as u64,
            height: 1,
            depth: 1
        };

        let threadgroup_size = {
            let max_threads = pipeline_state.max_total_threads_per_threadgroup();
            let width = if max_threads > array_length as u64 { array_length as u64 } else { max_threads };
            
            MTLSize {
                width: width as u64,
                height: 1,
                depth: 1
            }
        };

        compute_encoder.dispatch_threads(grid_size, threadgroup_size);
        compute_encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        verify_results(&buffer_a, &buffer_b, &result_buffer, array_length);
    });
}

fn generate_random_float_data(buffer: &BufferRef, length: usize) {
    let data_ptr = buffer.contents() as *mut f32;
    
    unsafe {
        for i in 0..length {
            *data_ptr.add(i) = rand::random::<f32>();
        }
    }
}

fn verify_results(buffer_a: &BufferRef, buffer_b: &BufferRef, result_buffer: &BufferRef, length: usize) {
    let a = buffer_a.contents() as *const f32;
    let b = buffer_b.contents() as *const f32;
    let result = result_buffer.contents() as *const f32;
    
    let mut success = true;
    
    unsafe {
        for i in 0..length {
            let a_val = *a.add(i);
            let b_val = *b.add(i);
            let result_val = *result.add(i);
            let expected = a_val + b_val;
            
            if (result_val - expected).abs() > 0.000001 {
                println!("Compute ERROR: index={} result={} vs {}=a+b", 
                         i, result_val, expected);
                success = false;
                break;
            }
        }
    }
    
    if success {
        println!("Compute results as expected");
    }
}

