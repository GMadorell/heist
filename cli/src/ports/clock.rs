use crate::domain::value::DateValue;

pub trait Clock {
    fn today(&self) -> DateValue;
}
