use mandelbrot_gpu::run;

fn main() {
    pollster::block_on(run());
}
