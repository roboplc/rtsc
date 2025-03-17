use std::collections::VecDeque;

use crate::data_policy::{DataDeliveryPolicy, DeliveryPolicy, StorageTryPushOutput};

/// A deque which stores values with respect of [`DataDeliveryPolicy`]
#[derive(Clone, Debug)]
pub struct Deque<T>
where
    T: DataDeliveryPolicy,
{
    data: VecDeque<T>,
    capacity: usize,
    ordered: bool,
}

impl<T> Deque<T>
where
    T: DataDeliveryPolicy,
{
    /// Creates a new bounded deque
    #[inline]
    pub fn bounded(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
            ordered: false,
        }
    }
    /// Enabled/disables priority ordering, can be used as a build pattern
    #[inline]
    pub fn set_ordering(mut self, v: bool) -> Self {
        self.ordered = v;
        self
    }
    /// Tries to store the value
    ///
    /// Returns the value back if there is no capacity even after all [`DataDeliveryPolicy`]
    /// rules have been applied
    ///
    /// Note: expired values are dropped and the operation returns: pushed=true
    pub fn try_push(&mut self, value: T) -> StorageTryPushOutput<T> {
        macro_rules! push {
            () => {{
                self.data.push_back(value);
                if self.ordered {
                    sort_by_priority(&mut self.data);
                }
                StorageTryPushOutput::Pushed
            }};
        }
        if value.is_expired() {
            return StorageTryPushOutput::Pushed;
        }
        if value.is_delivery_policy_single() {
            self.data.retain(|d| !d.eq_kind(&value) && !d.is_expired());
        }
        macro_rules! push_final {
            () => {
                if self.data.len() < self.capacity {
                    push!()
                } else {
                    StorageTryPushOutput::Full(value)
                }
            };
        }
        if self.data.len() < self.capacity {
            push!()
        } else {
            match value.delivery_policy() {
                DeliveryPolicy::Always | DeliveryPolicy::Single => {
                    let mut entry_removed = false;
                    self.data.retain(|d| {
                        if entry_removed {
                            true
                        } else if d.is_expired() || d.is_delivery_policy_optional() {
                            entry_removed = true;
                            false
                        } else {
                            true
                        }
                    });
                    push_final!()
                }
                DeliveryPolicy::Latest => {
                    let mut entry_removed = false;
                    self.data.retain(|d| {
                        if entry_removed {
                            true
                        } else if d.is_expired()
                            || d.is_delivery_policy_optional()
                            || d.eq_kind(&value)
                        {
                            entry_removed = true;
                            false
                        } else {
                            true
                        }
                    });
                    push_final!()
                }
                DeliveryPolicy::Optional | DeliveryPolicy::SingleOptional => {
                    StorageTryPushOutput::Skipped
                }
            }
        }
    }
    /// Returns the first available value, ignores expired ones
    #[inline]
    pub fn get(&mut self) -> Option<T> {
        loop {
            let value = self.data.pop_front();
            if let Some(ref val) = value {
                if !val.is_expired() {
                    break value;
                }
            } else {
                break None;
            }
        }
    }
    /// Clears the deque
    #[inline]
    pub fn clear(&mut self) {
        self.data.clear();
    }
    /// Returns number of elements in deque
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }
    /// Returns is the deque full
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity
    }
    /// Returns is the deque empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

fn sort_by_priority<T: DataDeliveryPolicy>(v: &mut VecDeque<T>) {
    v.rotate_right(v.as_slices().1.len());
    assert!(v.as_slices().1.is_empty());
    v.as_mut_slices()
        .0
        .sort_by(|a, b| a.priority().partial_cmp(&b.priority()).unwrap());
}

#[cfg(test)]
mod test {
    use super::Deque;
    use crate::data_policy::{DataDeliveryPolicy, StorageTryPushOutput};

    struct Data {
        id: u32,
        value: f64,
    }

    impl DataDeliveryPolicy for Data {
        fn delivery_policy(&self) -> crate::data_policy::DeliveryPolicy {
            crate::data_policy::DeliveryPolicy::Single
        }

        fn priority(&self) -> usize {
            100
        }

        fn eq_kind(&self, other: &Self) -> bool {
            self.id == other.id
        }

        fn is_expired(&self) -> bool {
            false
        }
    }

    #[test]
    fn test_dp_single() {
        let mut d: Deque<Data> = Deque::bounded(2);
        assert!(matches!(
            d.try_push(Data { id: 1, value: 1.0 }),
            StorageTryPushOutput::Pushed
        ));
        assert!(matches!(
            d.try_push(Data { id: 2, value: 2.0 }),
            StorageTryPushOutput::Pushed
        ));
        assert!(matches!(
            d.try_push(Data { id: 1, value: 3.0 }),
            StorageTryPushOutput::Pushed
        ));
        assert_eq!(d.len(), 2);
        let v1 = d.get().unwrap();
        assert_eq!(v1.id, 2);
        assert_eq!(v1.value, 2.0);
        let v2 = d.get().unwrap();
        assert_eq!(v2.id, 1);
        assert_eq!(v2.value, 3.0);
        assert!(d.get().is_none());
    }
}
