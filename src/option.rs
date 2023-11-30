/// If `other_value` has a value, overwrite `self_value`
pub fn overwrite_from_option<T: Clone>(self_value: &mut T, other_value: &Option<T>) {
    if let Some(value) = other_value.as_ref() {
        *self_value = value.clone();
    }
}

/// If `other_value` has a value, overwrite `self_value`
pub fn overwrite_option_from_option<T: Clone>(self_value: &mut Option<T>, other_value: &Option<T>) {
    if other_value.is_some() {
        *self_value = other_value.clone();
    }
}

/// If `a` is None, set it to `b`
pub fn update_if_none<T: Clone>(a: &mut Option<T>, b: &Option<T>) {
    if a.is_none() && b.is_some() {
        *a = b.clone();
    }
}
