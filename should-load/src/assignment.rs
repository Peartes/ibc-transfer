use cosmwasm_std::Storage;
use cw_storage_plus::{Map, PrimaryKey};
use serde::{de::DeserializeOwned, Serialize};

// we have our own trait written out down below. In the current state, it does not compile or provide the intended feature.
// Implement MapShouldLoad for Map without changing any trait bounds of the MapShouldLoad trait definition
// write sufficient testing to show you implementation works
// Where necessary, any other changes may be made

pub enum Error {
    KeyNotPresentInMap { key: Vec<u8> },
    EmptyValue { key: Vec<u8> },
    StdError(cosmwasm_std::StdError),
}

pub trait MapShouldLoad<'a, K, T, E>
where
    T: Serialize + DeserializeOwned,
{
    // should _load is used to shorten may_load to provide better errors and shorter dx for developers on non existent map values
    fn should_load(&self, storage: &mut dyn Storage, key: K) -> Result<T, E>;
}

impl<'a, K, T> MapShouldLoad<'a, K, T, Error> for Map<'a, K, T>
where
    K: PrimaryKey<'a> + Clone,
    T: Serialize + DeserializeOwned,
{
    fn should_load(&self, storage: &mut dyn Storage, key: K) -> Result<T, Error> {
        if self.has(storage, key.clone()) {
            let value = self
                .may_load(storage, key.clone())
                .map_err(|e| Error::StdError(e))?;

            match value {
                Some(value) => Ok(value),
                // We don't expect this to happen, but if it does, we want to know about it
                None => Err(Error::EmptyValue {
                    key: key.joined_key(),
                }),
            }
        } else {
            Err(Error::KeyNotPresentInMap {
                key: key.joined_key(),
            })
        }
    }
}

#[cfg(test)]
mod test {
    use cosmwasm_std::testing::mock_dependencies;
    use cw_storage_plus::Map;

    use super::{Error, MapShouldLoad};

    #[test]
    fn test_should_load_no_key() {
        let data: Map<&str, String> = Map::new("data");
        let mut deps = mock_dependencies();

        let res = data.should_load(deps.as_mut().storage, "unknown_key");
        match res {
            Err(Error::KeyNotPresentInMap { .. }) => {
                assert!(true)
            }
            _ => panic!("Unexpected error"),
        }
    }

    #[test]
    fn test_should_load_key() {
        let data: Map<&str, String> = Map::new("data");
        let mut deps = mock_dependencies();

        data.save(deps.as_mut().storage, "key", &"value".to_string())
            .unwrap();

        let res = data.should_load(deps.as_mut().storage, "key");
        match res {
            Ok(value) => {
                assert_eq!(value, "value".to_string())
            }
            _ => panic!("Unexpected error"),
        }
    }
}
