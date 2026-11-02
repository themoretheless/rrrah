//! Monotone pointer editing; no filesystem or GPU calls during a gesture.
use rrrah_core::edit::{EditState, MAX_CURVE_POINTS};
pub fn select_or_insert(state: &EditState, point: [f64; 2]) -> anyhow::Result<(EditState, usize)> {
    let mut points = state.points().to_vec();
    if let Some((index, _)) = points
        .iter()
        .enumerate()
        .filter(|(_, p)| ((p[0] - point[0]) * 256.).hypot((p[1] - point[1]) * 256.) <= 12.)
        .min_by(|(_, a), (_, b)| {
            ((a[0] - point[0]).hypot(a[1] - point[1])).total_cmp(&((b[0] - point[0]).hypot(b[1] - point[1])))
        })
    {
        return Ok((*state, index));
    }
    anyhow::ensure!(
        points.len() < MAX_CURVE_POINTS,
        "curve has 32 points; reset or move an existing point"
    );
    let index = points.partition_point(|p| p[0] < point[0]);
    anyhow::ensure!(index > 0 && index < points.len(), "select an existing endpoint");
    anyhow::ensure!(
        point[0] - points[index - 1][0] >= 1e-6 && points[index][0] - point[0] >= 1e-6,
        "curve points too close"
    );
    points.insert(
        index,
        [point[0], point[1].clamp(points[index - 1][1], points[index][1])],
    );
    Ok((state.with_curve(&points)?, index))
}
pub fn move_point(state: &EditState, index: usize, point: [f64; 2]) -> anyhow::Result<EditState> {
    let mut points = state.points().to_vec();
    anyhow::ensure!(index < points.len(), "point no longer belongs to this curve");
    let last = points.len() - 1;
    let x = if index == 0 || index == last {
        points[index][0]
    } else {
        {
            let low = points[index - 1][0] + 1.000_001e-6;
            let high = points[index + 1][0] - 1.000_001e-6;
            if low <= high {
                point[0].clamp(low, high)
            } else {
                points[index][0]
            }
        }
    };
    let y = point[1].clamp(
        if index == 0 { 0. } else { points[index - 1][1] },
        if index == last { 1. } else { points[index + 1][1] },
    );
    points[index] = [x, y];
    Ok(state.with_curve(&points)?)
}
/// Accessible button commands use the same monotone state contract as dragging.
pub fn keyboard_command(state: &EditState, selected: usize, id: u64) -> anyhow::Result<(EditState, usize)> {
    let selected = selected.min(state.points().len() - 1);
    match id {
        31 => Ok((*state, (selected + 1) % state.points().len())),
        32 => {
            let mut points = state.points().to_vec();
            anyhow::ensure!(points.len() < MAX_CURVE_POINTS, "curve has 32 points");
            let (index, pair) = points
                .windows(2)
                .enumerate()
                .max_by(|(_, a), (_, b)| (a[1][0] - a[0][0]).total_cmp(&(b[1][0] - b[0][0])))
                .unwrap();
            anyhow::ensure!(pair[1][0] - pair[0][0] > 2.000_01e-6, "curve points too close");
            let point = [(pair[0][0] + pair[1][0]) * 0.5, (pair[0][1] + pair[1][1]) * 0.5];
            points.insert(index + 1, point);
            Ok((state.with_curve(&points)?, index + 1))
        }
        33 => {
            let mut points = state.points().to_vec();
            anyhow::ensure!(
                selected > 0 && selected + 1 < points.len(),
                "curve endpoints cannot be removed"
            );
            points.remove(selected);
            Ok((state.with_curve(&points)?, selected.min(points.len() - 1)))
        }
        34..=37 => {
            let mut point = state.points()[selected];
            let axis = usize::from(id >= 36);
            point[axis] += if id == 34 || id == 36 { -0.01 } else { 0.01 };
            Ok((move_point(state, selected, point)?, selected))
        }
        38 => Ok((state.with_curve(EditState::default().points())?, 0)),
        _ => anyhow::bail!("unknown curve command"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_editing_covers_add_move_select_remove_reset_and_bounds() {
        let initial = EditState::default();
        let (mut state, mut selected) = keyboard_command(&initial, 0, 32).unwrap();
        assert_eq!(selected, 1);
        assert_eq!(state.points().len(), 3);
        for id in [34, 35, 36, 37] {
            (state, selected) = keyboard_command(&state, selected, id).unwrap();
            state.validate().unwrap();
        }
        let (state, selected) = keyboard_command(&state, selected, 33).unwrap();
        assert_eq!(state.points(), initial.points());
        assert!(keyboard_command(&state, 0, 33).is_err());
        let (_, next) = keyboard_command(&state, selected, 31).unwrap();
        assert_eq!(next, 0);
        let mut full = initial;
        for _ in 2..32 {
            (full, _) = keyboard_command(&full, 0, 32).unwrap();
        }
        assert!(keyboard_command(&full, 0, 32).is_err());
        assert_eq!(
            keyboard_command(&full, 0, 38).unwrap().0.points(),
            initial.points()
        );
    }
    #[test]
    fn dragging_stays_monotone_and_endpoint_x_stays_fixed() {
        let (s, i) = select_or_insert(&EditState::default(), [0.5, 0.7]).unwrap();
        assert_eq!(i, 1);
        let s = move_point(&s, i, [1., -1.]).unwrap();
        s.validate().unwrap();
        assert!(s.points()[1][0] < 1.);
        assert_eq!(s.points()[1][1], 0.);
        let s = move_point(&s, 0, [0.5, 1.]).unwrap();
        assert_eq!(s.points()[0][0], 0.);
        s.validate().unwrap();
    }
}
