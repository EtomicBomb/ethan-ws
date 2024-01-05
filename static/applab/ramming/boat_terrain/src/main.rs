extern crate png;

use std::path::Path;
use std::fs::File;
use std::io::BufWriter;
// To use encoder.set()
use png::HasParameters;

extern crate hsl;
use hsl::HSL;

extern crate rand;
use rand::thread_rng;
use rand::Rng;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use std::io::prelude::*;


const WIDTH: u32 = 320;
const HEIGHT: u32 = 450;
const TAU: f32 = 2.0*std::f32::consts::PI;

fn main() {
    let path = Path::new(r"out.png");
    let file = File::create(path).unwrap();
    let ref mut w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, WIDTH, HEIGHT); // Width is 2 pixels and height is 1.
    encoder.set(png::ColorType::RGB).set(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();

    let seed = thread_rng().gen_range(1000, 100000);

    let (perlin, collision_map) = generate_perlin(seed);
    let collision_map_string = encode_bitvec(collision_map);
    writer.write_image_data(&perlin.into_inner()).unwrap(); // Save
    
    let mut collision_map_file = File::create("collision_map.txt").unwrap();
    write!(collision_map_file, "{}", collision_map_string).unwrap();
    println!("{}", collision_map_string.len());
}

fn random_fraction(seed: u32) -> f32 {
    let x = (seed as f32).sin() * 10000.0;
    x - x.floor()
}

fn lerp(a0: f32, a1: f32, w: f32) -> f32 {
    let value = w * w * w * (w * (w * 6.0 - 15.0) + 10.0);
    a0 + value*(a1-a0)
}

fn dot_gradient(ix: u32, iy: u32, x: f32, y: f32, base_seed: u32) -> f32 {
    let mut hasher = DefaultHasher::new();
    (base_seed, ix, iy).hash(&mut hasher);
    let hash = hasher.finish() as u32 % 100000;
    let angle = TAU* random_fraction(hash);
    
    let gradient_x = angle.cos();
    let gradient_y = angle.sin();
    
    let grid_shift_x = x - ix as f32;
    let grid_shift_y = y - iy as f32;
    
    gradient_x * grid_shift_x + gradient_y * grid_shift_y
}

fn perlin(x: f32, y: f32, base_seed: u32) -> f32 {
    let ix = x.floor() as u32;
    let iy = y.floor() as u32;
    
    let wx = x - x.floor();
    let wy = y - y.floor();
    
    let n0 = lerp(
        dot_gradient(ix, iy, x, y, base_seed),
        dot_gradient(ix+1, iy, x, y, base_seed),    
        wx
    );
    
    let n1 = lerp(
        dot_gradient(ix, iy+1, x, y, base_seed),
        dot_gradient(ix+1, iy+1, x, y, base_seed),
        wx 
    );
    
    0.5 + lerp(n0, n1, wy)
}


fn encode_bitvec(vec: Vec<bool>) -> String {
    assert_eq!(0, vec.len()%32);
    let mut string = format!("[");
    for i in (0..vec.len()).step_by(32) {
        let mut n = 0;
        for inc in 0..32 {
            n += (vec[i+inc] as u32) << inc;
        }
        
        let dec_rep = format!("{},", n);
        let hex_rep = format!("0x{:x},", n);
        
        string += 
            if hex_rep.len() < dec_rep.len() {
                &hex_rep
            } else {
                &dec_rep
            };
    }
    /*
    // figure out how many we have left
    let left = vec.len() % 32;
    let offset = vec.len()-left;
    let mut n = 0;
    for inc in 0..left {
        n += (vec[offset+inc] as u32) << inc;
    }
    string += &format!("{}", n);
    */
    
    string += "]";
    string
}

fn generate_perlin(seed: u32) -> (Matrix, Vec<bool>) {
    let feature_size_constant = 4.0;
    
    let mut matrix = Matrix::new(WIDTH, HEIGHT);
    let mut collision_map = Vec::new();
    
    
    for iy in 0..HEIGHT {
        for ix in 0..WIDTH {
            let perlin_fraction = perlin(
                feature_size_constant*ix as f32 / WIDTH as f32,
                feature_size_constant*iy as f32 / HEIGHT as f32,
                seed,
            );
            
            let color = to_color(perlin_fraction);
            matrix.set_color(ix, iy, color);
            
            //let is_ground = perlin_fraction >= 0.65;
            let is_ground = false;
            collision_map.push(is_ground);
        }
    }
    
    (matrix, collision_map)
}



fn to_color(fraction: f32) -> RgbColor {
    /*
    let hsl = 
        if fraction < 0.65 {
            let d = (0.65- fraction)/(0.65*2.0);
            new_hsl(180.0, 1.0, 1.0-d/2.0 - 0.2)
        } else if fraction < 0.8 {
            let d = (0.8-fraction)/0.8;
            new_hsl(28.8, 0.42, d/2.0 + 0.6)
        } else {
            new_hsl(120.0, 0.88, 0.3-map(fraction, 0.8, 1.0, 0.0, 0.05))
        };
        */
        
    let d = (0.65- fraction)/(0.65*2.0);
    let hsl = new_hsl(180.0, 1.0, 1.0-d/2.0 - 0.2);
                
    RgbColor::from(hsl)
}



fn map(x: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    // https://www.arduino.cc/reference/en/language/functions/math/map/
    (x - in_min) * (out_max - out_min) / (in_max - in_min) + out_min
}


fn new_hsl(h: f32, s: f32, l: f32) -> HSL {
    HSL { h: h as f64, s: s as f64, l: l as f64 }
}


struct RgbColor {
    r: u8,
    g: u8,
    b: u8,
}


impl From<HSL> for RgbColor {
    fn from(source: HSL) -> RgbColor {
        let (r, g, b) = source.to_rgb();
        
        RgbColor { r, g, b }
    }
}

struct Matrix {
    width: u32,
    height: u32,
    inner: Vec<u8>,
}

impl Matrix {
    fn new(width: u32, height: u32) -> Matrix {
        let inner = vec![0; (width*height*3) as usize];
        Matrix { width, height, inner }
    }
    
    fn set_color(&mut self, x: u32, y: u32, color: RgbColor) {
        let start_index = (x*3 + y*self.width*3) as usize;
        self.inner[start_index] = color.r;
        self.inner[start_index+1] = color.g;
        self.inner[start_index+2] = color.b;
    }
    
    fn into_inner(self) -> Vec<u8> {
        self.inner
    }
}
