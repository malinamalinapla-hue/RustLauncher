use macroquad::prelude::*;
use std::collections::HashMap;

const W: usize = 48; // ширина мира (X)
const H: usize = 24; // высота (Y)
const D: usize = 48; // глубина (Z)
const CS: usize = 16; // размер чанка

// ---------- Мир ----------

struct World {
    blocks: Vec<u8>, // 0 = воздух, 1 = трава, 2 = земля, 3 = камень
}

impl World {
    fn new() -> Self {
        let mut blocks = vec![0u8; W * H * D];
        for z in 0..D {
            for x in 0..W {
                let h = (10.0 + (x as f32 * 0.25).sin() * 3.0 + (z as f32 * 0.18).cos() * 3.0)
                    as usize;
                for y in 0..=h.min(H - 1) {
                    blocks[(y * D + z) * W + x] = if y == h {
                        1
                    } else if y + 3 > h {
                        2
                    } else {
                        3
                    };
                }
            }
        }
        World { blocks }
    }

    fn inside(x: i32, y: i32, z: i32) -> bool {
        x >= 0 && y >= 0 && z >= 0 && (x as usize) < W && (y as usize) < H && (z as usize) < D
    }

    fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        if Self::inside(x, y, z) {
            self.blocks[(y as usize * D + z as usize) * W + x as usize]
        } else {
            0
        }
    }

    fn set(&mut self, x: i32, y: i32, z: i32, b: u8) {
        if Self::inside(x, y, z) {
            self.blocks[(y as usize * D + z as usize) * W + x as usize] = b;
        }
    }
}

// ---------- Меш ----------

struct Face {
    n: (i32, i32, i32),
    c: [[f32; 3]; 4],
    shade: f32,
}

const FACES: [Face; 6] = [
    Face { n: (1, 0, 0), c: [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]], shade: 0.8 },
    Face { n: (-1, 0, 0), c: [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]], shade: 0.8 },
    Face { n: (0, 1, 0), c: [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]], shade: 1.0 },
    Face { n: (0, -1, 0), c: [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]], shade: 0.5 },
    Face { n: (0, 0, 1), c: [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]], shade: 0.65 },
    Face { n: (0, 0, -1), c: [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]], shade: 0.65 },
];

fn block_color(b: u8) -> Color {
    match b {
        1 => Color::new(0.30, 0.65, 0.25, 1.0),
        2 => Color::new(0.50, 0.35, 0.20, 1.0),
        _ => Color::new(0.50, 0.50, 0.52, 1.0),
    }
}

/// Строит меш одного чанка: рисуются только грани, соседствующие с воздухом.
fn build_chunk(w: &World, cx: usize, cz: usize) -> Mesh {
    let mut vertices: Vec<Vertex> = vec![];
    let mut indices: Vec<u16> = vec![];
    for y in 0..H {
        for z in cz * CS..(cz + 1) * CS {
            for x in cx * CS..(cx + 1) * CS {
                let b = w.get(x as i32, y as i32, z as i32);
                if b == 0 {
                    continue;
                }
                let base = block_color(b);
                for f in FACES.iter() {
                    let (nx, ny, nz) = f.n;
                    if w.get(x as i32 + nx, y as i32 + ny, z as i32 + nz) != 0 {
                        continue;
                    }
                    let s = f.shade;
                    let col = Color::new(base.r * s, base.g * s, base.b * s, 1.0);
                    let i0 = vertices.len() as u16;
                    for c in f.c.iter() {
                        vertices.push(Vertex::new(
                            x as f32 + c[0],
                            y as f32 + c[1],
                            z as f32 + c[2],
                            0.0,
                            0.0,
                            col,
                        ));
                    }
                    indices.extend_from_slice(&[i0, i0 + 1, i0 + 2, i0, i0 + 2, i0 + 3]);
                }
            }
        }
    }
    Mesh { vertices, indices, texture: None }
}

fn build_world(w: &World) -> Vec<Mesh> {
    let mut meshes = vec![];
    for cz in 0..D / CS {
        for cx in 0..W / CS {
            meshes.push(build_chunk(w, cx, cz));
        }
    }
    meshes
}

// ---------- Выбор блока лучом ----------

/// Возвращает (блок, в который попали; последняя пустая клетка перед ним).
fn raycast(w: &World, origin: Vec3, dir: Vec3) -> Option<(IVec3, IVec3)> {
    let mut prev = origin.floor().as_ivec3();
    let mut t = 0.0;
    while t < 8.0 {
        let c = (origin + dir * t).floor().as_ivec3();
        if w.get(c.x, c.y, c.z) != 0 {
            return Some((c, prev));
        }
        prev = c;
        t += 0.05;
    }
    None
}

// ---------- Ввод ----------

struct Ptr {
    start: Vec2,
    last: Vec2,
    t0: f64,
    moved: f32,
    left: bool, // левая половина экрана = движение, правая = взгляд и тапы
}

#[macroquad::main("Voxel Game")]
async fn main() {
    let mut world = World::new();
    let mut meshes = build_world(&world);

    let mut pos = vec3(24.0, 18.0, 24.0);
    let mut yaw: f32 = 0.0;
    let mut pitch: f32 = -0.4;
    let mut place_mode = false;
    let mut ptrs: HashMap<u64, Ptr> = HashMap::new();

    loop {
        let dt = get_frame_time();
        let sw = screen_width();
        let button = Rect::new(sw - 130.0, 10.0, 120.0, 50.0);

        // Собираем события: касания, а на ПК вместо них мышь.
        let mut events: Vec<(u64, TouchPhase, Vec2)> =
            touches().iter().map(|t| (t.id, t.phase, t.position)).collect();
        if events.is_empty() {
            let m = Vec2::from(mouse_position());
            if is_mouse_button_pressed(MouseButton::Left) {
                events.push((100, TouchPhase::Started, m));
            } else if is_mouse_button_down(MouseButton::Left) {
                events.push((100, TouchPhase::Moved, m));
            } else if is_mouse_button_released(MouseButton::Left) {
                events.push((100, TouchPhase::Ended, m));
            }
        }

        let front = vec3(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos());

        for (id, phase, p) in events {
            match phase {
                TouchPhase::Started => {
                    ptrs.insert(
                        id,
                        Ptr { start: p, last: p, t0: get_time(), moved: 0.0, left: p.x < sw * 0.5 },
                    );
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    if let Some(t) = ptrs.remove(&id) {
                        let is_tap = !t.left && t.moved < 15.0 && get_time() - t.t0 < 0.3;
                        if is_tap {
                            if button.contains(p) {
                                place_mode = !place_mode;
                            } else if let Some((hit, prev)) = raycast(&world, pos, front) {
                                if place_mode {
                                    if prev != pos.floor().as_ivec3() {
                                        world.set(prev.x, prev.y, prev.z, 3);
                                    }
                                } else {
                                    world.set(hit.x, hit.y, hit.z, 0);
                                }
                                meshes = build_world(&world);
                            }
                        }
                    }
                }
                _ => {
                    if let Some(t) = ptrs.get_mut(&id) {
                        let d = p - t.last;
                        t.moved += d.length();
                        t.last = p;
                        if !t.left {
                            yaw += d.x * 0.005;
                            pitch = (pitch - d.y * 0.005).clamp(-1.5, 1.5);
                        }
                    }
                }
            }
        }

        // Движение: виртуальный джойстик слева + WASD на ПК. Пока это свободный полёт.
        let mut mv = Vec2::ZERO; // x = вправо, y = вперёд
        for t in ptrs.values().filter(|t| t.left) {
            mv += ((t.last - t.start) / 80.0).clamp_length_max(1.0) * vec2(1.0, -1.0);
        }
        if is_key_down(KeyCode::W) { mv.y += 1.0; }
        if is_key_down(KeyCode::S) { mv.y -= 1.0; }
        if is_key_down(KeyCode::D) { mv.x += 1.0; }
        if is_key_down(KeyCode::A) { mv.x -= 1.0; }
        if is_key_pressed(KeyCode::B) { place_mode = !place_mode; }
        let mv = mv.clamp_length_max(1.0);
        let right = front.cross(vec3(0.0, 1.0, 0.0)).normalize();
        pos += (front * mv.y + right * mv.x) * 6.0 * dt;

        // ---------- Рисование ----------
        clear_background(Color::new(0.5, 0.7, 1.0, 1.0));
        set_camera(&Camera3D {
            position: pos,
            target: pos + front,
            up: vec3(0.0, 1.0, 0.0),
            ..Default::default()
        });
        for m in &meshes {
            draw_mesh(m);
        }

        set_default_camera();
        let (cx, cy) = (sw / 2.0, screen_height() / 2.0);
        draw_line(cx - 10.0, cy, cx + 10.0, cy, 2.0, WHITE);
        draw_line(cx, cy - 10.0, cx, cy + 10.0, 2.0, WHITE);
        draw_rectangle(button.x, button.y, button.w, button.h, Color::new(0.0, 0.0, 0.0, 0.5));
        draw_text(if place_mode { "PLACE" } else { "BREAK" }, button.x + 15.0, button.y + 33.0, 30.0, WHITE);
        for t in ptrs.values().filter(|t| t.left) {
            draw_circle_lines(t.start.x, t.start.y, 60.0, 3.0, WHITE);
            draw_circle(t.last.x, t.last.y, 25.0, Color::new(1.0, 1.0, 1.0, 0.5));
        }

        next_frame().await;
    }
              }
