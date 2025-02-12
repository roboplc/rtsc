use std::{collections::BTreeMap, fmt, ops::Sub};

/// A map which contains events happened at some key points.
#[derive(Default)]
pub struct EventMap<K, V, D> {
    data: BTreeMap<K, V>,
    max_delta: Option<D>,
}

impl<K, V, D> fmt::Debug for EventMap<K, V, D>
where
    K: fmt::Debug,
    V: fmt::Debug,
    D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventMap")
            .field("data", &self.data)
            .field("max_delta", &self.max_delta)
            .finish()
    }
}

/// Event value
pub struct EventValue<'a, K, D, V> {
    key: K,
    value: &'a V,
    delta: D,
}

impl<K, D, V> EventValue<'_, K, D, V>
where
    K: Clone,
    D: Clone,
{
    /// Returns the actual key point of the event.
    pub fn key(&self) -> K {
        self.key.clone()
    }
    /// Returns the value of the event.
    pub fn value(&self) -> &V {
        self.value
    }
    /// Returns the delta between the requested key and the event key point.
    pub fn delta(&self) -> D {
        self.delta.clone()
    }
    /// Consumes the event and returns the value.
    pub fn into_value(self) -> V
    where
        V: Clone,
    {
        self.value.clone()
    }
}

impl<K, D, V> fmt::Debug for EventValue<'_, K, D, V>
where
    K: fmt::Debug,
    D: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventValue")
            .field("value", &self.value)
            .field("key", &self.key)
            .field("delta", &self.delta)
            .finish()
    }
}

struct EventKey<K, D> {
    key: K,
    delta: D,
}

impl<K, V, D> EventMap<K, V, D>
where
    K: Clone + Ord + Sub<Output = D>,
    D: PartialOrd,
{
    /// Creates a new empty event map.
    pub fn new() -> Self {
        EventMap {
            data: BTreeMap::new(),
            max_delta: None,
        }
    }
    /// Limits the maximum delta between the requested key and the event key point.
    pub fn with_max_delta(mut self, max_delta: D) -> Self {
        self.max_delta = Some(max_delta);
        self
    }
    /// Inserts a new event at the specified key point.
    pub fn insert(&mut self, key: K, value: V) {
        self.data.insert(key, value);
    }
    /// Gets the most closest event to the specified key point.
    pub fn get_closest_to(&self, key: K) -> Option<EventValue<K, D, V>> {
        let lower: Option<K> = self
            .data
            .range(..=key.clone())
            .next_back()
            .map(|(k, _)| k)
            .cloned();
        let upper: Option<K> = self
            .data
            .range(key.clone()..)
            .next()
            .map(|(k, _)| k)
            .cloned();
        let closest = match (lower, upper) {
            (Some(l), Some(u)) => {
                let lower_diff = key.clone() - l.clone();
                let upper_diff = u.clone() - key;
                if lower_diff <= upper_diff {
                    Some(EventKey {
                        key: l,
                        delta: lower_diff,
                    })
                } else {
                    Some(EventKey {
                        key: u,
                        delta: upper_diff,
                    })
                }
            }
            (Some(l), None) => Some(EventKey {
                key: l.clone(),
                delta: key - l,
            }),
            (None, Some(u)) => Some(EventKey {
                key: u.clone(),
                delta: u - key,
            }),
            (None, None) => None,
        };
        if let Some(ref max_delta) = self.max_delta {
            if let Some(ref closest) = closest {
                if closest.delta > *max_delta {
                    return None;
                }
            }
        }
        closest.and_then(|closest| {
            self.data.get(&closest.key).map(|value| EventValue {
                key: closest.key,
                value,
                delta: closest.delta,
            })
        })
    }
    /// Clears the event map.
    pub fn clear_all(&mut self) {
        self.data.clear();
    }
    /// Cleanup map, deleting events before the specified key point.
    pub fn cleanup(&mut self, key: K) {
        self.data = self.data.split_off(&key);
    }
    /// Get event data map.
    pub fn data(&self) -> &BTreeMap<K, V> {
        &self.data
    }
    /// Get mutable event data map.
    pub fn data_mut(&mut self) -> &mut BTreeMap<K, V> {
        &mut self.data
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn test_event_map() {
        let mut event_map = super::EventMap::new();
        event_map.insert(1, "a");
        event_map.insert(3, "b");
        event_map.insert(7, "d");
        event_map.insert(9, "e");
        let event = event_map.get_closest_to(4).unwrap();
        assert_eq!(event.key(), 3);
        assert_eq!(event.value(), &"b");
        assert_eq!(event.delta(), 1);
        let event = event_map.get_closest_to(5).unwrap();
        assert_eq!(event.key(), 3);
        assert_eq!(event.value(), &"b");
        assert_eq!(event.delta(), 2);
        let event = event_map.get_closest_to(6).unwrap();
        assert_eq!(event.key(), 7);
        assert_eq!(event.value(), &"d");
        assert_eq!(event.delta(), 1);
        let event = event_map.get_closest_to(10).unwrap();
        assert_eq!(event.key(), 9);
        assert_eq!(event.value(), &"e");
        assert_eq!(event.delta(), 1);
        let event = event_map.get_closest_to(100).unwrap();
        assert_eq!(event.key(), 9);
        assert_eq!(event.value(), &"e");
        assert_eq!(event.delta(), 91);
        event_map = event_map.with_max_delta(91);
        let event = event_map.get_closest_to(100).unwrap();
        assert_eq!(event.key(), 9);
        let event = event_map.get_closest_to(-90).unwrap();
        assert_eq!(event.key(), 1);
        assert_eq!(event.value(), &"a");
        assert_eq!(event.delta(), 91);
        let event = event_map.get_closest_to(-100);
        assert!(event.is_none());
        event_map = event_map.with_max_delta(90);
        let event = event_map.get_closest_to(-90);
        assert!(event.is_none());
        let event = event_map.get_closest_to(100);
        assert!(event.is_none());
        assert_eq!(event_map.data().len(), 4);
        event_map.cleanup(7);
        assert_eq!(event_map.data().len(), 2);
    }
}
