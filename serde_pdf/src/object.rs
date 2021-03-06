use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use crate::stream::Stream;
use crate::{
    ser::{NAME_OBJECT, NAME_REFERENCE},
    Value,
};
use serde::{ser::SerializeTupleStruct, Serialize, Serializer};
use std::io;
use std::rc::Rc;

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct ObjectId(Rc<ObjectIdInner>);

#[derive(PartialEq, Eq, Hash)]
struct ObjectIdInner {
    id: usize,
    rev: usize,
}

pub struct Object<D = ()>
where
    D: Serialize,
{
    id: ObjectId,
    content: D,
}

pub struct Reference<D>(ObjectId, PhantomData<D>)
where
    D: Serialize;

impl<D> Object<D>
where
    D: Serialize,
{
    pub fn new(id: usize, rev: usize, content: D) -> Self {
        Object {
            id: ObjectId::new(id, rev),
            content,
        }
    }

    pub fn id(&self) -> usize {
        self.id.id()
    }

    pub fn rev(&self) -> usize {
        self.id.rev()
    }

    pub fn to_reference(&self) -> Reference<D> {
        Reference::new(self.id.clone())
    }

    pub fn content_mut(&mut self) -> &mut D {
        &mut self.content
    }
}

impl Default for Object<()> {
    fn default() -> Self {
        Object {
            id: ObjectId::new(0, 0),
            content: (),
        }
    }
}

impl ObjectId {
    pub fn new(id: usize, rev: usize) -> Self {
        ObjectId(Rc::new(ObjectIdInner { id, rev }))
    }

    pub fn id(&self) -> usize {
        self.0.id
    }

    pub fn rev(&self) -> usize {
        self.0.rev
    }
}

impl<D: Serialize> io::Write for Object<Stream<D>> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        (&mut self.content).write(buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        (&mut self.content).flush()
    }
}

impl<D> Serialize for Object<D>
where
    D: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_tuple_struct(NAME_OBJECT, 3)?;
        s.serialize_field(&self.id.id())?;
        s.serialize_field(&self.id.rev())?;
        s.serialize_field(&self.content)?;
        s.end()
    }
}

impl<D> Reference<D>
where
    D: Serialize,
{
    pub fn new(id: ObjectId) -> Self {
        Reference(id, PhantomData)
    }
}

impl<D: Serialize> Clone for Reference<D> {
    fn clone(&self) -> Self {
        Reference(self.0.clone(), PhantomData)
    }
}

impl<D> Serialize for Reference<D>
where
    D: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_tuple_struct(NAME_REFERENCE, 2)?;
        s.serialize_field(&(self.0).id())?;
        s.serialize_field(&(self.0).rev())?;
        s.end()
    }
}

impl<D> Hash for Reference<D>
where
    D: Serialize,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<D> PartialEq for Reference<D>
where
    D: Serialize,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<D> Eq for Reference<D> where D: Serialize {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ser::to_string;

    #[test]
    fn test_serialize_object() {
        let obj: Object<()> = Object {
            id: ObjectId::new(3, 1),
            ..Object::default()
        };
        assert_eq!(to_string(&obj).unwrap(), "3 1 obj\nnull\nendobj\n\n");
    }

    #[test]
    fn test_serialize_reference() {
        let obj: Object<()> = Object {
            id: ObjectId::new(3, 1),
            ..Object::default()
        };
        let reference = obj.to_reference();
        assert_eq!(to_string(&reference).unwrap(), "3 1 R");
    }
}
