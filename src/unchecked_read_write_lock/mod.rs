use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::ops::{Deref, DerefMut};

#[derive(Debug)]
pub struct UncheckedRwLock<T>(RwLock<T>);

impl<T> Deref for UncheckedRwLock<T>{
    type Target = RwLock<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for UncheckedRwLock<T>{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> UncheckedRwLock<T>{
    pub fn into_read_write_lock(self) -> RwLock<T> {
        self.0
    }

    pub fn new(read_write_lock : RwLock<T>) -> Self {
        Self{0: read_write_lock}
    }

    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        self.0.read().unwrap()
    }


    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.0.write().unwrap()
    }
}


impl<T: Default> Default for UncheckedRwLock<T> {
    fn default() -> UncheckedRwLock<T> {
        Self{0: RwLock::new(Default::default())}
    }
}

impl<T> From<T> for UncheckedRwLock<T> {
    fn from(t: T) -> Self {
        Self{0: RwLock::new(t)}
    }
}
