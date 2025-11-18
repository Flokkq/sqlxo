#[macro_export]
macro_rules! and {
    ( $( $e:expr ),* $(,)? ) => {
        $crate::Expression::And(vec![
            $( $crate::Expression::from($e) ),*
        ])
    };
}

#[macro_export]
macro_rules! or {
    ( $( $e:expr ),* $(,)? ) => {
        $crate::Expression::Or(vec![
            $( $crate::Expression::from($e) ),*
        ])
    };
}

#[macro_export]
macro_rules! order_by {
    ( $( $e:expr ),+ $(,)? ) => {{
        let mut v = ::std::vec::Vec::new();
        $(
            v.extend($e.into_iter());
        )+
        <$crate::SortOrder<_> as ::core::convert::From<::std::vec::Vec<_>>>
            ::from(v)
    }};
    () => {
        <$crate::SortOrder<_> as ::core::convert::From<::std::vec::Vec<_>>>
            ::from(::std::vec::Vec::new())
    };
}
