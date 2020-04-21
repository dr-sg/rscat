use super::Vertex;

pub fn get_default_vertices() -> Vec<Vertex> {
    let mut vertices = Vec::<Vertex>::new();
    for x_idx in -1000..1000 {
        for y_idx in -1000..1000 {
            let x = x_idx as f32 /10.0;
            let y = y_idx as f32/10.0;
            let distance = (x * x + y * y).sqrt();
            let z = if distance == 0_f32 {
                1_f32
            } else {
                distance.sin() / distance
            } * 10.0;
            let angle = y.atan2(x);
            vertices.push(Vertex {
                position: [x, y, z, 1.0],
                color: [angle/3.1415/2.0+0.5, -angle/3.1415/2.0+0.5, z / 10_f32, 1.0],
                size: 1.0,
            });
        }
    }

    // Test - One million and thirty one points!
    return vertices;
}

pub fn axes() -> Vec<Vertex> {
    let mut vertices = Vec::<Vertex>::new();
    vertices.push(Vertex {
        position: [0.0, 0.0, 0.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        size: 40.0,
    });
    for i in 1..10 {
        vertices.push(Vertex {
            position: [i as f32, 0.0, 0.0, 1.0],
            color: [1.0, 0.0, 0.0, 0.0],
            size: 20.0,
        });
        vertices.push(Vertex {
            position: [0.0, i as f32, 0.0, 1.0],
            color: [0.0, 1.0, 0.0, 0.0],
            size: 20.0,
        });
        vertices.push(Vertex {
            position: [0.0, 0.0, i as f32, 1.0],
            color: [0.0, 0.0, 1.0, 0.0],
            size: 20.0,
        });
    }
    return vertices;
}

pub fn render_all_vertices(vertices: &Vec<Vertex>) -> Vec<u32> {
    return (0..vertices.len() as u32).collect();
}
