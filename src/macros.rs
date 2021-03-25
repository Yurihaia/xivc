#[macro_export]
macro_rules! action_combo {
    (
        $(
            $(#[$attrs:meta])*
            $v:vis enum $name:ident {
                $($action:ident),* $(,)?
            }
        )*
    ) => {
        $(
            $(#[$attrs])*
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

#[macro_export]
macro_rules! action_cooldown {
    (
        $(#[$attrs:meta])*
        $v:vis enum $name:ident : $parent:path {
            $($action:ident),* $(,)?
        }
    ) => {
        $(#[$attrs])*
        $v enum $name {
            $(
                $action,
            )*
        }

        impl ::std::convert::From<$name> for $parent {
            fn from(e: $name) -> Self {
                match e {
                    $(
                        $name::$action => <$parent>::$action,
                    )*
                }
            }
        }

        impl ::std::convert::TryFrom<$parent> for $name {
            type Error = $parent;

            fn try_from(value: $parent) -> Result<Self, Self::Error> {
                match value {
                    $(
                        <$parent>::$action => Ok(Self::$action),
                    )*
                    v => Err(v)
                }
            }
        }
    }
}