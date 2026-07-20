use super::{BezierNode, BezierPath, DocumentPoint, NodeKind};

impl BezierPath {
    pub(super) fn insert_node_on_segment(&mut self, start_index: usize, t: f32) -> Option<usize> {
        if self.nodes.len() < 2 || start_index >= self.nodes.len() {
            return None;
        }
        let end_index = if start_index + 1 < self.nodes.len() {
            start_index + 1
        } else if self.closed {
            0
        } else {
            return None;
        };
        let t = t.clamp(0.001, 0.999);
        let start = self.nodes[start_index];
        let end = self.nodes[end_index];
        let first = interpolate_point(start.position, start.handle_out, t);
        let middle = interpolate_point(start.handle_out, end.handle_in, t);
        let last = interpolate_point(end.handle_in, end.position, t);
        let handle_in = interpolate_point(first, middle, t);
        let handle_out = interpolate_point(middle, last, t);
        let position = interpolate_point(handle_in, handle_out, t);

        self.nodes[start_index].handle_out = first;
        self.nodes[end_index].handle_in = last;
        if self.nodes[start_index].kind == NodeKind::Symmetric {
            self.nodes[start_index].kind = NodeKind::Smooth;
        }
        if self.nodes[end_index].kind == NodeKind::Symmetric {
            self.nodes[end_index].kind = NodeKind::Smooth;
        }
        let node = BezierNode {
            position,
            handle_in,
            handle_out,
            kind: NodeKind::Smooth,
        };
        if end_index == 0 {
            self.nodes.push(node);
            Some(self.nodes.len() - 1)
        } else {
            self.nodes.insert(end_index, node);
            Some(end_index)
        }
    }

    pub(super) fn set_node_kinds(&mut self, indices: &[usize], kind: NodeKind) {
        let snapshot = self.nodes.clone();
        for &index in indices {
            let Some(node) = self.nodes.get_mut(index) else {
                continue;
            };
            node.kind = kind;
            if kind == NodeKind::Corner {
                continue;
            }
            let direction = node_tangent(&snapshot, self.closed, index);
            let (incoming, outgoing) = node_neighbors(&snapshot, self.closed, index)
                .map(|(previous, next)| {
                    (
                        point_distance(node.position, previous.position) / 3.0,
                        point_distance(node.position, next.position) / 3.0,
                    )
                })
                .unwrap_or_default();
            align_node_handles(
                node,
                direction,
                incoming,
                outgoing,
                kind == NodeKind::Symmetric,
            );
        }
    }

    pub(super) fn smooth_nodes(&mut self, indices: &[usize]) {
        let snapshot = self.nodes.clone();
        for &index in indices {
            let Some(node) = self.nodes.get_mut(index) else {
                continue;
            };
            let Some((previous, next)) = node_neighbors(&snapshot, self.closed, index) else {
                continue;
            };
            let direction = normalize(DocumentPoint::new(
                next.position.x - previous.position.x,
                next.position.y - previous.position.y,
            ));
            let incoming = point_distance(node.position, previous.position) / 3.0;
            let outgoing = point_distance(node.position, next.position) / 3.0;
            node.handle_in = DocumentPoint::new(
                node.position.x - direction.x * incoming,
                node.position.y - direction.y * incoming,
            );
            node.handle_out = DocumentPoint::new(
                node.position.x + direction.x * outgoing,
                node.position.y + direction.y * outgoing,
            );
            node.kind = NodeKind::Smooth;
        }
    }

    pub(super) fn remove_nodes_preserving_shape(&mut self, indices: &[usize]) {
        let mut indices = indices.to_vec();
        indices.sort_unstable();
        indices.dedup();
        for index in indices.into_iter().rev() {
            self.remove_node_preserving_shape(index);
        }
    }

    fn remove_node_preserving_shape(&mut self, index: usize) {
        if index >= self.nodes.len() {
            return;
        }
        if self.nodes.len() <= 2 {
            self.nodes.remove(index);
            self.closed = false;
            return;
        }
        if !self.closed && (index == 0 || index + 1 == self.nodes.len()) {
            self.nodes.remove(index);
            if let Some(first) = self.nodes.first_mut() {
                first.handle_in = first.position;
            }
            if let Some(last) = self.nodes.last_mut() {
                last.handle_out = last.position;
            }
            return;
        }

        let previous_index = if index == 0 {
            self.nodes.len() - 1
        } else {
            index - 1
        };
        let next_index = (index + 1) % self.nodes.len();
        let previous = self.nodes[previous_index];
        let removed = self.nodes[index];
        let next = self.nodes[next_index];
        let (handle_out, handle_in) = fit_joined_cubics(previous, removed, next);
        self.nodes[previous_index].handle_out = handle_out;
        self.nodes[next_index].handle_in = handle_in;
        self.nodes[previous_index].kind = NodeKind::Corner;
        self.nodes[next_index].kind = NodeKind::Corner;
        self.nodes.remove(index);
    }
}

pub(super) fn simplification_candidates(
    path: &BezierPath,
    selected: &[usize],
    tolerance: f32,
) -> Vec<usize> {
    let mut candidates = selected
        .iter()
        .copied()
        .filter(|index| {
            if *index >= path.nodes.len()
                || (!path.closed && (*index == 0 || *index + 1 == path.nodes.len()))
            {
                return false;
            }
            let previous = if *index == 0 {
                path.nodes[path.nodes.len() - 1]
            } else {
                path.nodes[*index - 1]
            };
            let node = path.nodes[*index];
            let next = path.nodes[(*index + 1) % path.nodes.len()];
            let (handle_out, handle_in) = fit_joined_cubics(previous, node, next);
            (1..8).all(|step| {
                let t = step as f32 / 8.0;
                point_distance(
                    joined_cubic_point(previous, node, next, t),
                    cubic_point(previous.position, handle_out, handle_in, next.position, t),
                ) <= tolerance
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    let mut previous = None;
    candidates.retain(|index| {
        let keep = previous.is_none_or(|previous| *index > previous + 1);
        if keep {
            previous = Some(*index);
        }
        keep
    });
    candidates
}

fn interpolate_point(first: DocumentPoint, second: DocumentPoint, t: f32) -> DocumentPoint {
    DocumentPoint::new(
        first.x + (second.x - first.x) * t,
        first.y + (second.y - first.y) * t,
    )
}

fn node_neighbors(
    nodes: &[BezierNode],
    closed: bool,
    index: usize,
) -> Option<(BezierNode, BezierNode)> {
    let node = *nodes.get(index)?;
    let previous = if index > 0 {
        nodes[index - 1]
    } else if closed {
        *nodes.last()?
    } else {
        node
    };
    let next = if index + 1 < nodes.len() {
        nodes[index + 1]
    } else if closed {
        nodes[0]
    } else {
        node
    };
    Some((previous, next))
}

fn node_tangent(nodes: &[BezierNode], closed: bool, index: usize) -> DocumentPoint {
    let Some((previous, next)) = node_neighbors(nodes, closed, index) else {
        return DocumentPoint::new(1.0, 0.0);
    };
    normalize(DocumentPoint::new(
        next.position.x - previous.position.x,
        next.position.y - previous.position.y,
    ))
}

fn normalize(vector: DocumentPoint) -> DocumentPoint {
    let length = (vector.x * vector.x + vector.y * vector.y).sqrt();
    if length <= f32::EPSILON {
        DocumentPoint::new(1.0, 0.0)
    } else {
        DocumentPoint::new(vector.x / length, vector.y / length)
    }
}

fn align_node_handles(
    node: &mut BezierNode,
    direction: DocumentPoint,
    fallback_in: f32,
    fallback_out: f32,
    symmetric: bool,
) {
    let mut incoming = point_distance(node.position, node.handle_in);
    let mut outgoing = point_distance(node.position, node.handle_out);
    if incoming <= f32::EPSILON {
        incoming = fallback_in;
    }
    if outgoing <= f32::EPSILON {
        outgoing = fallback_out;
    }
    if symmetric {
        let length = (incoming + outgoing) / 2.0;
        incoming = length;
        outgoing = length;
    }
    node.handle_in = DocumentPoint::new(
        node.position.x - direction.x * incoming,
        node.position.y - direction.y * incoming,
    );
    node.handle_out = DocumentPoint::new(
        node.position.x + direction.x * outgoing,
        node.position.y + direction.y * outgoing,
    );
}

fn fit_joined_cubics(
    previous: BezierNode,
    removed: BezierNode,
    next: BezierNode,
) -> (DocumentPoint, DocumentPoint) {
    let mut a = 0.0;
    let mut b = 0.0;
    let mut c = 0.0;
    let mut right_1 = DocumentPoint::default();
    let mut right_2 = DocumentPoint::default();
    for step in 1..8 {
        let t = step as f32 / 8.0;
        let target = joined_cubic_point(previous, removed, next, t);
        let inverse = 1.0 - t;
        let basis_0 = inverse.powi(3);
        let basis_1 = 3.0 * inverse.powi(2) * t;
        let basis_2 = 3.0 * inverse * t.powi(2);
        let basis_3 = t.powi(3);
        let residual = DocumentPoint::new(
            target.x - basis_0 * previous.position.x - basis_3 * next.position.x,
            target.y - basis_0 * previous.position.y - basis_3 * next.position.y,
        );
        a += basis_1 * basis_1;
        b += basis_1 * basis_2;
        c += basis_2 * basis_2;
        right_1.x += basis_1 * residual.x;
        right_1.y += basis_1 * residual.y;
        right_2.x += basis_2 * residual.x;
        right_2.y += basis_2 * residual.y;
    }
    let determinant = a * c - b * b;
    if determinant.abs() <= f32::EPSILON {
        return (previous.handle_out, next.handle_in);
    }
    (
        DocumentPoint::new(
            (right_1.x * c - right_2.x * b) / determinant,
            (right_1.y * c - right_2.y * b) / determinant,
        ),
        DocumentPoint::new(
            (a * right_2.x - b * right_1.x) / determinant,
            (a * right_2.y - b * right_1.y) / determinant,
        ),
    )
}

fn joined_cubic_point(
    previous: BezierNode,
    removed: BezierNode,
    next: BezierNode,
    t: f32,
) -> DocumentPoint {
    if t <= 0.5 {
        cubic_point(
            previous.position,
            previous.handle_out,
            removed.handle_in,
            removed.position,
            t * 2.0,
        )
    } else {
        cubic_point(
            removed.position,
            removed.handle_out,
            next.handle_in,
            next.position,
            t * 2.0 - 1.0,
        )
    }
}

fn cubic_point(
    start: DocumentPoint,
    first: DocumentPoint,
    second: DocumentPoint,
    end: DocumentPoint,
    t: f32,
) -> DocumentPoint {
    let inverse = 1.0 - t;
    DocumentPoint::new(
        start.x * inverse.powi(3)
            + first.x * 3.0 * inverse.powi(2) * t
            + second.x * 3.0 * inverse * t.powi(2)
            + end.x * t.powi(3),
        start.y * inverse.powi(3)
            + first.y * 3.0 * inverse.powi(2) * t
            + second.y * 3.0 * inverse * t.powi(2)
            + end.y * t.powi(3),
    )
}

fn point_distance(first: DocumentPoint, second: DocumentPoint) -> f32 {
    ((first.x - second.x).powi(2) + (first.y - second.y).powi(2)).sqrt()
}
