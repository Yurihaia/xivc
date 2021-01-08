#[macro_export]
macro_rules! action_combo {
    (
        $(
            $(#[$attrs:meta])?
            $v:vis enum $name:ident {
                $($action:ident),* $(,)?
            }
        )*
    ) => {
        $(
            $(#[$attrs])?
            $v enum $name {
                None,
                $(
                    $action (u16)
                ),*
            }
    
            impl $name {
                pub fn advance(&mut self, time: u32) {
                    match self {
                        $(Self::$action (v) )|* => {
                            *v = (*v as u32).saturating_sub(time) as u16;
                        },
                        Self::None => ()
                    }
                }
            }
        )*
    }
}