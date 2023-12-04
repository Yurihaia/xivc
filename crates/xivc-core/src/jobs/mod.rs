pub mod rpr;
pub mod sam;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use core::fmt;

macro_rules! helper {
    (
        $($enum_name:ident $var_name:ident $job:ty)*
    ) => {
        helper!(
            #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
            #[derive(Copy, Clone, Debug)]
            enum Action, $(
                $enum_name, <$job as $crate::job::Job>::Action, $job, $var_name
            )*
        );
        helper!(
            #[derive(Clone, Debug)]
            enum Error, $(
                $enum_name, <$job as $crate::job::Job>::Error, $job, $var_name
            )*
        );
        impl fmt::Display for Error {
            fn fmt(
                &self,
                f: &mut fmt::Formatter<'_>
            ) -> Result<(), fmt::Error> {
                match self {
                    $(
                        Self::$var_name(v) => write!(
                            f,
                            "{}: {}",
                            $crate::enums::Job::$enum_name.name(),
                            v,
                        ),
                    )*
                }
            }
        }
        helper!(
            #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
            #[derive(Clone, Debug)]
            enum State, $(
                $enum_name, <$job as $crate::job::Job>::State, $job, $var_name
            )*
        );
        helper!(
            #[derive(Clone, Debug)]
            enum JobEvent, nofrom, $(
                $enum_name, <$job as $crate::job::Job>::Event, $job, $var_name
            )*
        );
    };
    (
        $(#[$m:meta])*
        enum $whole_name:ident, nofrom, $($enum_name:ident, $job_type:ty, $job:ty, $var_name:ident)*
    ) => {
        $(#[$m])*
        pub enum $whole_name {
            $(
                $var_name($job_type),
            )*
        }

        impl $whole_name {
            pub fn job(&self) -> $crate::enums::Job {
                match self {
                    $(Self::$var_name(_) => $crate::enums::Job::$enum_name,)*
                }
            }
        }
    };
    (
        $(#[$m:meta])*
        enum $whole_name:ident, $($enum_name:ident, $job_type:ty, $job:ty, $var_name:ident)*
    ) => {
        helper!(
            $(#[$m])*
            enum $whole_name, nofrom, $($enum_name, $job_type, $job, $var_name)*
        );
        $(
            impl From<$job_type> for $whole_name {
                fn from(val: $job_type) -> Self {
                    Self::$var_name(val)
                }
            }
        )*
    }
}

helper! {
    SAM Sam sam::SamJob
    RPR Rpr rpr::RprJob
}
