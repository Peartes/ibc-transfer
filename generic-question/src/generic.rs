
// Write a function that accepts a collection of any kind, loops of that collection and applies a function f given by the caller
// the function f is generic over the elements in the collection and returns a Result of a generic ok value and a generic error value

// write the function here
pub fn apply_to_collection<T, Ok, Err, F, I>(collection: I, f: F) -> Vec<Result<Ok, Err>>
where
    F: Fn(&mut T) -> Result<Ok, Err>,
    I: IntoIterator<Item = T>,
{
    collection.into_iter().map(|mut item| f(&mut item)).collect::<Vec<Result<Ok, Err>>>()
}

#[cfg(test)]
mod test {
    use crate::generic::apply_to_collection;

    #[test]
    // test on a collection of integers
    fn test_collection_integer() {
        let numbers = vec![1, 2, 3, 4, 5];
    
        // A function that doubles a number and returns it as a Result
        fn double_number(num: &mut i32) -> Result<i32, &'static str> {
            if *num < 0 {
                Err("Number cannot be negative")
            } else {
                *num = *num * 2;
                Ok(*num)
            }
        }
    
        // Call the function with the numbers and the double_number function
        let result: Vec<Result<i32, &str>> = apply_to_collection(numbers, double_number);
    
        // convert the result to a vector of integers
        // It is left to the developer to handle errors in unwrapping. This is not ideal, but it is a limitation of the exercise
        // For this test case we know that the function will not return an error
        let result = result.iter().map(|x| x.clone().unwrap()).collect::<Vec<i32>>();
        // make sure the result is correct
        assert_eq!(result, vec![2, 4, 6, 8, 10])
    }

    #[test]
    // test on a collection of strings
    fn test_collection_string() {
        let strings = vec!["hello".to_string(), "world".to_string(), "foo".to_string(), "bar".to_string()];
        // A function that puts strings in uppercase
        fn uppercase_string(string: &mut String) -> Result<String, &'static str> {
            *string = string.to_uppercase();
            Ok(string.clone())
        }
    
        // Call the function with the numbers and the double_number function
        let result: Vec<Result<String, &str>> = apply_to_collection(strings, uppercase_string);

        // convert the result to a vector of strings
        let result = result.iter().map(|x| x.clone().unwrap()).collect::<Vec<String>>();
        // make sure the result is correct
        assert_eq!(result, vec!["HELLO".to_string(), "WORLD".to_string(), "FOO".to_string(), "BAR".to_string()])
    }

    #[test]
    // test on a map of integers to strings
    fn test_collection_map() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert(1, "hello".to_string());
        map.insert(2, "world".to_string());
        map.insert(3, "foo".to_string());
        map.insert(4, "bar".to_string());

        // A function that puts strings in uppercase
        fn uppercase_string(val: &mut (i32, String)) -> Result<(i32, String), &'static str> {
            Ok((val.0, val.1.to_uppercase()))
        }
    
        // Call the function with the numbers and the double_number function
        let result: Vec<Result<(i32, String), &str>> = apply_to_collection(map, uppercase_string);

        // convert the result to a hashmap of strings
        let result = result.iter().map(|x| x.clone().unwrap()).collect::<HashMap<i32, String>>();
        // make sure the result is correct
        let mut expected = HashMap::new();
        expected.insert(1, "HELLO".to_string());
        expected.insert(2, "WORLD".to_string());
        expected.insert(3, "FOO".to_string());
        expected.insert(4, "BAR".to_string());
        assert_eq!(result, expected)
    }
}