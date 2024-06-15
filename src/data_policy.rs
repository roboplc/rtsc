use core::fmt;
use std::{mem, str::FromStr};

use crate::{Result, Error};

/// Data delivery policies, used by [`crate::pdeque::Deque`]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum DeliveryPolicy {
    #[default]
    /// always deliver, fail if no room (default)
    Always,
    /// always deliver, drop the previous if no room (act as a ring-buffer)
    Latest,
    /// skip delivery if no room
    Optional,
    /// always deliver the frame but always in a single copy (latest)
    Single,
    /// deliver a single latest copy, skip if no room
    SingleOptional,
}

impl FromStr for DeliveryPolicy {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "always" => Ok(DeliveryPolicy::Always),
            "optional" => Ok(DeliveryPolicy::Optional),
            "single" => Ok(DeliveryPolicy::Single),
            "single-optional" => Ok(DeliveryPolicy::SingleOptional),
            _ => Err(Error::InvalidData(s.to_string())),
        }
    }
}

impl fmt::Display for DeliveryPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DeliveryPolicy::Always => "always",
                DeliveryPolicy::Latest => "latest",
                DeliveryPolicy::Optional => "optional",
                DeliveryPolicy::Single => "single",
                DeliveryPolicy::SingleOptional => "single-optional",
            }
        )
    }
}

/// Implements delivery policies for own data types
pub trait DataDeliveryPolicy
where
    Self: Sized,
{
    /// Delivery policy, the default is [`DeliveryPolicy::Always`]
    fn delivery_policy(&self) -> DeliveryPolicy {
        DeliveryPolicy::Always
    }
    /// Priority, for ordered, lower is better, the default is 100
    fn priority(&self) -> usize {
        100
    }
    /// Has equal kind with other
    ///
    /// (default: check enum discriminant)
    fn eq_kind(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
    /// If a frame expires during storing/delivering, it is not delivered
    fn is_expired(&self) -> bool {
        false
    }
    #[doc(hidden)]
    fn is_delivery_policy_single(&self) -> bool {
        let dp = self.delivery_policy();
        dp == DeliveryPolicy::Single || dp == DeliveryPolicy::SingleOptional
    }
    #[doc(hidden)]
    fn is_delivery_policy_optional(&self) -> bool {
        let dp = self.delivery_policy();
        dp == DeliveryPolicy::Optional || dp == DeliveryPolicy::SingleOptional
    }
}

/// Result payload of try_push operation
pub enum StorageTryPushOutput<T: Sized> {
    /// the value is pushed
    Pushed,
    /// the value is skipped
    Skipped,
    /// the value has been skipped
    Full(T),
}
