// The MIT License (MIT)
//
// Copyright (c) 2015 Aaron Loucks <aloucks+github@cofront.net>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

use std::slice::Iter;
use std::hash::Hash;
use std::hash::Hasher;
use std::fmt::{self, Debug};
use std::ops::Index;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Entity(u64);

const INVALID_ID: u64 = std::u64::MAX;
const INVALID_INDEX: u32 = std::u32::MAX;

impl Default for Entity {
    #[inline(always)]
    fn default() -> Entity {
        Entity(INVALID_ID)
    }
}

impl Debug for Entity {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Entity({{id: {}, key: {}, gen: {}}})", self.0, self.key(), self.gen())
    }
}

impl Hash for Entity {
    #[inline(always)]
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        state.write_u64(self.0)
    }
}

impl Entity {
    #[inline(always)]
    fn from_key_and_gen(key: u32, gen: u32) -> Entity {
        Entity(((key as u64) << 32) | (gen as u64))
    }

    #[inline(always)]
    fn key(&self) -> u32 {
        (self.0 >> 32) as u32
    }

    #[inline(always)]
    fn gen(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }
}

#[derive(Debug, Clone)]
pub struct EntityPool {
    entities: Vec<Entity>,
    entities_free: Vec<Entity>,
    entity_index: Vec<u32>, // entity_index[entity.key] => index; entities[index as usize]
    next_entity_key: u32,
}

impl Default for EntityPool {
    #[inline(always)]
    fn default() -> EntityPool {
        EntityPool::new()
    }
}

impl EntityPool {

    /// Creates a new, empty `EntityPool`.
    ///
    /// The `EntityPool` will not allocate until entities are created.
    ///
    pub fn new() -> EntityPool {
        EntityPool::with_capacity(0, 0)
    }

    /// Creates a new, empty `EntityPool` with the specified capacities.
    ///
    /// The `EntityPool` will able to create `create_capacity` and return `return_capacity`
    /// entities without reallocating. If either capacity is 0, their respective storage
    /// vectors will not allocate.
    pub fn with_capacity(create_capacity: usize, return_capacity: usize) -> EntityPool {
        EntityPool {
            entities: Vec::with_capacity(create_capacity),
            entities_free: Vec::with_capacity(return_capacity),
            entity_index: Vec::with_capacity(create_capacity),
            next_entity_key: 0
        }
    }

    /// Creates a unique entity.
    ///
    /// Returns the `Entity` and it's current `index`. The index is only guaranteed to remain
    /// valid until the next call to `return_entity`.
    pub fn create_entity(&mut self) -> (usize, Entity) {
        let (key, gen) = match self.entities_free.pop() {
            Some(entity) => {
                (entity.key(), entity.gen().wrapping_add(1))
            },
            None => {
                let key = self.next_entity_key;
                self.next_entity_key = key + 1;
                (key, 0)
            }
        };
        let entity = Entity::from_key_and_gen(key, gen);
        let index = self.entities.len() as u32;
        self.entities.push(entity);
        if key as usize != self.entity_index.len() {
            self.entity_index[key as usize] = index;
        }
        else {
            debug_assert_eq!(key as usize, self.entity_index.len());
            self.entity_index.push(index);
        }
        (index as usize, entity)
    }

    /// Release ownership of the `entity`, allowing for it to be recycled. A recycled entity will
    /// have it's internal generation incremented, yielding a new, unique entity.
    ///
    /// Entities are stored in contiguous memory. When an entity is returned, the last entity is
    /// swaped into the returned entity's slot; thus indexes retrieved prior to returning an
    /// entity are potentially invalidated.
    ///
    /// # Panics
    ///
    /// Returning an entity more than once, or an entity created from another pool, results in
    /// undefined behavior.
    ///
    /// # Examples
    ///
    /// ```
    /// use entitypool::EntityPool;
    ///
    /// let mut pool = EntityPool::new();
    /// let (_, e1) = pool.create_entity();
    /// pool.return_entity(e1);
    /// let (_, e2) = pool.create_entity();
    /// assert!(e1 != e2);
    /// ```
    ///
    /// ```
    /// use entitypool::EntityPool;
    ///
    /// let mut pool = EntityPool::new();
    /// let (i1, e1) = pool.create_entity();
    /// let (i2, e2) = pool.create_entity();
    /// let (i3, e3) = pool.create_entity();
    /// assert_eq!(0, i1);
    /// assert_eq!(1, i2);
    /// assert_eq!(2, i3);
    /// assert_eq!(0, pool.index_of(e1));
    /// assert_eq!(1, pool.index_of(e2));
    /// assert_eq!(2, pool.index_of(e3));
    /// pool.return_entity(e2);
    /// assert_eq!(0, pool.index_of(e1));
    /// assert_eq!(1, pool.index_of(e3));
    /// ```
    pub fn return_entity(&mut self, entity: Entity) {
        debug_assert!(entity != Entity::default());
        let key = entity.key();
        let index = self.entity_index[key as usize];
        debug_assert_eq!(entity.gen(), self.entities[index as usize].gen());
        self.entities_free.push(entity);
        self.entities.swap_remove(index as usize);
        self.entity_index[key as usize] = INVALID_INDEX;
        match self.entities.get(index as usize) {
            Some(e) => self.entity_index[e.key() as usize] = index,
            None    => {}
        };
    }

    /// Returns the current `index` of the given `entity`, which is only guaranteed to remain
    /// valid until the next call to `return_entity`.
    ///
    /// # Panics
    ///
    /// Querying the status of an entity from another pool results in undefined behavior.
    #[inline(always)]
    pub fn index_of(&self, entity: Entity) -> usize {
        debug_assert!(entity != Entity::default());
        let key = entity.key();
        let index = self.entity_index[key as usize] as usize;
        debug_assert_eq!(entity.gen(), self.entities[index as usize].gen());
        index
    }

    /// Returns the current `entity` at the given `index`.
    ///
    /// # Panics
    ///
    /// Panics if the index is greater or equal to the number of live entities in this pool.
    #[inline(always)]
    pub fn entity_at(&self, index: usize) -> Entity {
        self.entities[index]
    }

    /// Returns `true` if this entity has not been returned.
    ///
    /// # Panics
    ///
    /// Querying the status of an entity from another pool results in undefined behavior.
    pub fn is_alive(&self, entity: Entity) -> bool {
        debug_assert!(entity != Entity::default());
        let key = entity.key();
        let index = self.entity_index[key as usize];
        if index != INVALID_INDEX {
            let other = self.entities[index as usize];
            key == other.key() && entity.gen() == other.gen()
        }
        else {
            false
        }
    }

    /// Returns an iterator to the live entities. The `Enumerate` of the returned iterator will
    /// yield each `entity` and its current `index`.
    ///
    /// # Examples
    ///
    /// ```
    /// use entitypool::EntityPool;
    ///
    /// let mut pool = EntityPool::new();
    /// pool.create_entity();
    /// pool.create_entity();
    /// pool.create_entity();
    /// for (index, entity) in pool.iter().enumerate() {
    ///     assert_eq!(index, pool.index_of(*entity));
    ///     assert_eq!(*entity, pool.entity_at(index));
    /// }
    /// ```
    #[inline(always)]
    pub fn iter(&self) -> Iter<Entity> {
        self.entities.iter()
    }

    /// Resets the `EnitityPool` to its initial state, without releasing allocated capacity.
    ///
    /// All entities created prior to resetting are no longer considered members of this pool.
    pub fn reset(&mut self) {
        self.entities.clear();
        self.entities_free.clear();
        self.entity_index.clear();
        self.next_entity_key = 0;
    }

    /// Reserves capacity for at least `additional` more entities to be created without
    /// reallocation. The pool may reserve more space to avoid frequesnt reallocations.
    pub fn reserve(&mut self, additional: usize) {
        self.entities.reserve(additional);
        self.entity_index.reserve(additional);
    }

    /// Reserves capacity for at least `additional` more entities to be returned without
    /// reallocation. The pool may reserve more space to avoid frequesnt reallocations.
    pub fn reserve_returned(&mut self, additional: usize) {
        self.entities_free.reserve(additional);
    }

    /// Shrinks the capacity of this pool as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.entities.shrink_to_fit();
        self.entities_free.shrink_to_fit();
        self.entity_index.shrink_to_fit();
    }

    /// Returns the number of live entities in this pool.
    #[inline(always)]
    pub fn len(&self) -> usize {
        debug_assert_eq!(self.entities.len(), self.entity_index.len());
        self.entities.len()
    }

    /// Returns the number of returned entities in this pool that are ready to be recycled.
    #[inline(always)]
    pub fn len_returned(&self) -> usize {
        self.entities_free.len()
    }

    /// Returns the number of entities that this pool can create without reallocation.
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        debug_assert_eq!(self.entities.capacity(), self.entity_index.capacity());
        self.entities.capacity()
    }

    /// Returns the number of entities that can be returned without reallocation.
    #[inline(always)]
    pub fn capacity_returned(&self) -> usize {
        self.entities_free.capacity()
    }
}

impl Index<u32> for EntityPool {
    type Output = Entity;
    /// Returns the `entity` at the given `index`.
    #[inline(always)]
    fn index(&self, index: u32) -> &Entity {
        &self.entities[index as usize]
    }
}

impl Index<Entity> for EntityPool {
    type Output = u32;
    /// Returns the index of the given `entity`.
    #[inline(always)]
    fn index(&self, entity: Entity) -> &u32 {
        &self.entity_index[entity.key() as usize]
    }
}

#[test]
fn it_works() {
    let mut pool = EntityPool::new();
    let mut entities = Vec::<Entity>::new();
    for i in 0..5 {
        let (index, e) = pool.create_entity();
        assert_eq!(i, index);
        assert_eq!(e, pool.entity_at(index));
        assert_eq!(e, pool[i as u32]);
        assert_eq!(index, pool.index_of(e));
        assert_eq!(index as u32, pool[e]);
        assert!(!entities.contains(&e));
        assert!(pool.is_alive(e));
        entities.push(e);
    }
    let mut alive = 5;
    for e in entities.iter() {
        pool.return_entity(*e);
        assert!(!pool.is_alive(*e));
        alive -= 1;
        let mut expected_alive = 0;
        for (i_alive, e_alive) in pool.iter().enumerate() {
            assert!(pool.is_alive(*e_alive));
            assert_eq!(i_alive, pool.index_of(*e_alive));
            assert_eq!(i_alive as u32, pool[*e_alive]);
            assert_eq!(*e_alive, pool.entity_at(i_alive));
            assert_eq!(*e_alive, pool[i_alive as u32]);
            expected_alive += 1;
        }
        assert_eq!(expected_alive, alive);
    }
    for i in 0..5 {
        let (index, e) = pool.create_entity();
        assert_eq!(i, index);
        assert_eq!(e, pool.entity_at(index));
        assert_eq!(e, pool[i as u32]);
        assert_eq!(index, pool.index_of(e));
        assert_eq!(index as u32, pool[e]);
        assert!(!entities.contains(&e));
        assert!(pool.is_alive(e));
        entities.push(e);
    }
    let mut count = 0;
    for (index, e) in pool.iter().enumerate() {
        assert_eq!(index, pool.index_of(*e));
        assert_eq!(index as u32, pool[*e]);
        assert_eq!(*e, pool.entity_at(index));
        assert_eq!(*e, pool[index as u32]);
        assert!(pool.is_alive(*e));
        count += 1;
    }
    assert_eq!(5, count);
}
