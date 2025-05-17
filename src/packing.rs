use bevy::{
    math::{Mat2, Vec2},
    prelude::Deref,
};

/// A polygon represented by its edge vectors (CCW order).
#[derive(Deref, Clone, Debug)]
pub struct EdgeVectors(pub Vec<Vec2>);

impl EdgeVectors {
    pub fn with_rect_size_rotation(size: Vec2, rotation: f32, div: Option<u8>) -> Self {
        let rotation_matrix = Mat2::from_angle(rotation);

        // Ordered so that the first point is the bottom-left most
        let mut points = vec![
            rotation_matrix * Vec2::new(size.x, 0.0),
            rotation_matrix * Vec2::new(0.0, size.y),
            rotation_matrix * Vec2::new(-size.x, 0.0),
            rotation_matrix * Vec2::new(0.0, -size.y),
        ];

        if let Some(div) = div {
            let old_points = points;
            points = Vec::with_capacity(old_points.len() * div as usize);
            for p in old_points {
                for _ in 0..div {
                    points.push(p / div as f32);
                }
            }
        }

        EdgeVectors(points)
    }

    /// Construct from a list of vertices.
    pub fn from_vertices(vertices: &[Vec2]) -> Self {
        let mut edges = Vec::with_capacity(vertices.len());
        for i in 0..vertices.len() {
            let a = vertices[i];
            let b = vertices[(i + 1) % vertices.len()];
            edges.push(b - a);
        }
        EdgeVectors(edges)
    }

    /// Return the iterator of vertices. The initial vertex is (0, 0).
    fn local_vertices(&self) -> impl Iterator<Item = Vec2> {
        self.0.iter().scan(Vec2::ZERO, |acc, edge| {
            *acc += *edge;
            Some(*acc)
        })
    }
}

/// Compute the Minkowski sum of two polygons.
fn minkowski_sum(a: &EdgeVectors, b: &EdgeVectors) -> EdgeVectors {
    // https://cp-algorithms.com/geometry/minkowski.html

    // Get the index of bottom-left most point
    fn get_first_index(v: impl Iterator<Item = Vec2>) -> usize {
        v.into_iter()
            .enumerate()
            .min_by(|(_, v), (_, w)| {
                (v.y, v.x)
                    .partial_cmp(&(w.y, w.x))
                    .expect("NaN element in EdgeVectors")
            })
            .unwrap()
            .0
    }

    let mut i = get_first_index(a.local_vertices());
    let mut j = get_first_index(b.local_vertices());

    let mut i_inc = 0;
    let mut j_inc = 0;

    let mut result = Vec::with_capacity(a.len().max(b.len()));

    let mut cur = Vec2::ZERO;

    // Iterate until we have traversed all edges of both shapes
    while i_inc < a.len() || j_inc < b.len() {
        let a_i = a[i];
        let b_j = b[j];

        cur += a_i + b_j;
        result.push(cur);

        let cross = a_i.perp_dot(b_j);

        if cross >= 0.0 && i_inc < a.len() {
            i = (i + 1) % a.len();
            i_inc += 1;
        }
        if cross <= 0.0 && j_inc < b.len() {
            j = (j + 1) % b.len();
            j_inc += 1;
        }
    }

    EdgeVectors::from_vertices(&result)
}

#[derive(Clone, Debug)]
pub struct ShapePosition {
    pub translation: Vec2,
    pub edges: EdgeVectors,
}

impl ShapePosition {
    pub fn vertices(&self) -> Vec<Vec2> {
        // local vertices
        let mut vertices = self.edges.local_vertices().collect::<Vec<_>>();

        let centroid = calculate_centroid(&vertices);

        // translate to world space
        vertices.iter_mut().for_each(|v| {
            *v += self.translation;
            *v -= centroid;
        });

        vertices
    }

    fn offset(&mut self, width: f32) {
        let vertices = self.vertices();

        let mut new_vertices = vertices.clone();

        for i in 0..self.edges.len() {
            // Outward normal
            let prev_normal = -self.edges[i].perp().normalize();
            let next_normal = -self.edges[(i + 1) % self.edges.len()].perp().normalize();

            new_vertices[i] = vertices[i] + (prev_normal + next_normal) * width;
        }

        self.edges = EdgeVectors::from_vertices(&new_vertices);
        self.translation -= calculate_centroid(&new_vertices) - calculate_centroid(&vertices);
    }
}

// TODO: add `gap` paremeter
pub fn fill<'a>(
    placed_shapes: impl IntoIterator<Item = &'a ShapePosition> + Clone,
    shape_to_place: &ShapePosition,
    offset: f32,
) -> ShapePosition {
    let mut candidates = vec![];

    for placed in placed_shapes.clone() {
        let nfp = minkowski_sum(&placed.edges, &shape_to_place.edges);

        let mut nfp_shape = ShapePosition {
            translation: placed.translation,
            edges: nfp,
        };
        nfp_shape.offset(offset);
        let mut nfp_vertices = nfp_shape.vertices();

        // Sort by distance to initial translation of `shape_to_place`
        nfp_vertices.sort_by(|v, w| {
            (v - shape_to_place.translation)
                .length()
                .partial_cmp(&(w - shape_to_place.translation).length())
                .expect("NaN")
        });

        for nfp_vertex in nfp_vertices {
            // A vertex on the Minkowski sum is a candidate for the new shape position.
            // If the translated shape is not inside any of the placed shapes, add it to candidates.

            let mut inside = false;

            // TODO: use a spatial partitioning to speed this up
            for placed2 in placed_shapes.clone() {
                let placed2_vertices = placed2.vertices();

                for edge in shape_to_place
                    .edges
                    .local_vertices()
                    .map(|v| v + nfp_vertex)
                {
                    if is_inside(&placed2_vertices, edge) {
                        inside = true;
                        break;
                    }
                }
            }

            if !inside {
                let translated_shape = ShapePosition {
                    translation: nfp_vertex,
                    edges: shape_to_place.edges.clone(),
                };
                candidates.push(translated_shape);
            }
        }
    }

    // Needs heuristic (like, choosing from 4 directions based on original translation)
    // For now, choose one that is nearest to original translation
    candidates.sort_by(|a, b| {
        (a.translation - shape_to_place.translation)
            .length()
            .partial_cmp(&(b.translation - shape_to_place.translation).length())
            .expect("NaN")
    });

    candidates.swap_remove(0)
}

/// Check if a point is inside a polygon (convex, CCW vertex list).
fn is_inside(polygon: &[Vec2], point: Vec2) -> bool {
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];

        let cross = (b - a).perp_dot(point - a);

        if cross < 0.0 {
            return false;
        }
    }

    true
}

fn calculate_centroid(vertices: &[Vec2]) -> Vec2 {
    let mut centroid = Vec2::ZERO;
    for vertex in vertices {
        centroid += *vertex;
    }
    centroid / vertices.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_inside() {
        let polygon = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 4.0),
            Vec2::new(0.0, 4.0),
        ];

        assert!(is_inside(&polygon, Vec2::new(2.0, 2.0)));
        assert!(!is_inside(&polygon, Vec2::new(5.0, 5.0)));
        assert!(!is_inside(&polygon, Vec2::new(2.0, 5.0)));
        assert!(!is_inside(&polygon, Vec2::new(-2.0, 2.0)));
    }

    #[test]
    fn test_minkowski_sum() {
        let a = EdgeVectors::with_rect_size_rotation(Vec2::new(4.0, 3.0), 0.0, None);
        let b = EdgeVectors::with_rect_size_rotation(Vec2::new(2.0, 1.0), 0.0, None);

        let result = minkowski_sum(&a, &b);

        assert_eq!(result.len(), 4);
        assert_eq!(result[0], Vec2::new(6.0, 0.0));
        assert_eq!(result[1], Vec2::new(0.0, 4.0));
        assert_eq!(result[2], Vec2::new(-6.0, 0.0));
        assert_eq!(result[3], Vec2::new(0.0, -4.0));

        let a = EdgeVectors::with_rect_size_rotation(Vec2::new(4.0, 3.0), 0.0, Some(2));
        let b = EdgeVectors::with_rect_size_rotation(Vec2::new(2.0, 1.0), 0.0, Some(2));

        let result = minkowski_sum(&a, &b);

        assert_eq!(result.len(), 8);
        assert_eq!(result[0], Vec2::new(3.0, 0.0));
        assert_eq!(result[1], Vec2::new(3.0, 0.0));
        assert_eq!(result[2], Vec2::new(0.0, 2.0));
        assert_eq!(result[3], Vec2::new(0.0, 2.0));
        assert_eq!(result[4], Vec2::new(-3.0, 0.0));
        assert_eq!(result[5], Vec2::new(-3.0, 0.0));
        assert_eq!(result[6], Vec2::new(0.0, -2.0));
        assert_eq!(result[7], Vec2::new(0.0, -2.0));
    }

    #[test]
    fn test_fill() {
        let placed_shapes = vec![ShapePosition {
            translation: Vec2::new(0.0, 0.0),
            edges: EdgeVectors::with_rect_size_rotation(Vec2::new(4.0, 4.0), 0.0, None),
        }];

        let shape_to_place = ShapePosition {
            translation: Vec2::new(25.0, 25.0),
            edges: EdgeVectors::with_rect_size_rotation(Vec2::new(2.0, 2.0), 0.0, None),
        };

        let result = fill(&placed_shapes, &shape_to_place, 0.1);

        // Ensure the result is not overlapping with the placed shape
        for placed in &placed_shapes {
            for vertex in result.vertices() {
                assert!(!is_inside(&placed.vertices(), vertex));
            }
        }

        let gap = 0.1;

        assert!(
            (result.translation.length() - 3.0 * std::f32::consts::SQRT_2 - gap) < 0.1,
            "{:?}",
            result.translation
        );

        // Since we initially placed the shape at (25, 25), the result should be close to that
        assert!(result.translation.cmpgt(Vec2::new(0.0, 0.0)).all());
    }
}
