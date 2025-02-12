use std::{collections::BTreeMap, fmt, ops::Sub};

/// A map which contains events happened at some key points.
#[derive(Default)]
pub struct EventMap<K, V> {
    data: BTreeMap<K, V>,
}

impl<K, V> fmt::Debug for EventMap<K, V>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventMap")
            .field("data", &self.data)
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

impl<K, V, D> EventMap<K, V>
where
    K: Clone + Ord + Sub<Output = D>,
    D: PartialOrd,
{
    /// Creates a new empty event map.
    pub fn new() -> Self {
        EventMap {
            data: BTreeMap::new(),
        }
    }
    /// Inserts a new event at the specified key point.
    pub fn insert(&mut self, key: K, value: V) {
        self.data.insert(key, value);
    }
    /// Gets the most closest event to the specified key point.
    pub fn get_closest_to(&mut self, key: K) -> Option<EventValue<K, D, V>> {
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
        closest.and_then(|closest| {
            self.data.get(&closest.key).map(|value| EventValue {
                key: closest.key,
                value,
                delta: closest.delta,
            })
        })
    }
    /// Clears the event map.
    pub fn clear(&mut self) {
        self.data.clear();
    }
    /// Removes the event at the specified key point.
    pub fn remove(&mut self, key: K) -> Option<V> {
        self.data.remove(&key)
    }
}
