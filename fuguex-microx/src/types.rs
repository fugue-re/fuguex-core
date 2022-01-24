#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ViolationSource {
    Read,
    Write,
    ReadVia,
    WriteVia,
}

pub enum HookInvalidAccessAction<R, V> {
    Pass,
    Skip, // for writes
    Halt(R),
    Value(Vec<V>), // for reads
}

impl<R, V> HookInvalidAccessAction<R, V> {
    pub fn is_value(&self) -> bool {
        matches!(self, Self::Value(_))
    }

    pub fn is_halt(&self) -> bool {
        matches!(self, Self::Halt(_))
    }

    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }

    pub fn is_skip(&self) -> bool {
        matches!(self, Self::Skip)
    }

    pub fn value(&self) -> Option<&[V]> {
        if let Self::Value(ref v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn into_value(self) -> Option<Vec<V>> {
        if let Self::Value(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn unwrap_value(self) -> Vec<V> {
        self.into_value().unwrap()
    }
}
