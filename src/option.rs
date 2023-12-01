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

#[cfg(test)]
mod test {
    mod overwrite_from_option {
        use super::super::overwrite_from_option;
        #[test]
        fn with_some() {
            let mut a = 1;
            let b = Some(2);
            overwrite_from_option(&mut a, &b);
            assert_eq!(a, 2);
        }

        #[test]
        fn with_none() {
            let mut a = 1;
            let b = None;
            overwrite_from_option(&mut a, &b);
            assert_eq!(a, 1);
        }
    }

    mod overwrite_option_from_option {
        use super::super::overwrite_option_from_option;

        #[test]
        fn some_with_some() {
            let mut a = Some(1);
            let b = Some(2);
            overwrite_option_from_option(&mut a, &b);
            assert_eq!(a, Some(2));
        }

        #[test]
        fn some_with_none() {
            let mut a = Some(1);
            let b = None;
            overwrite_option_from_option(&mut a, &b);
            assert_eq!(a, Some(1));
        }

        #[test]
        fn none_with_some() {
            let mut a = None;
            let b = Some(2);
            overwrite_option_from_option(&mut a, &b);
            assert_eq!(a, Some(2));
        }

        #[test]
        fn none_with_none() {
            let mut a: Option<usize> = None;
            let b = None;
            overwrite_option_from_option(&mut a, &b);
            assert_eq!(a, None);
        }
    }

    mod update_if_none {
        use super::super::update_if_none;

        #[test]
        fn some_with_some() {
            let mut a = Some(1);
            let b = Some(2);
            update_if_none(&mut a, &b);
            assert_eq!(a, Some(1));
        }

        #[test]
        fn some_with_none() {
            let mut a = Some(1);
            let b = None;
            update_if_none(&mut a, &b);
            assert_eq!(a, Some(1));
        }

        #[test]
        fn none_with_some() {
            let mut a = None;
            let b = Some(2);
            update_if_none(&mut a, &b);
            assert_eq!(a, Some(2));
        }

        #[test]
        fn none_with_none() {
            let mut a: Option<usize> = None;
            let b = None;
            update_if_none(&mut a, &b);
            assert_eq!(a, None);
        }
    }
}
