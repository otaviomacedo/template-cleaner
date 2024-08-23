mod bimap;
mod union_find;

use crate::union_find::UnionFind;
use include_dir::{include_dir, Dir};
use itertools::Itertools;
use std::collections::HashMap;
use yaml_rust2::yaml::Hash;
use yaml_rust2::{Yaml, YamlLoader};

const SCHEMAS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/resources/schemas");

fn clean_template(template: &String) -> anyhow::Result<Yaml> {
    let mut docs = YamlLoader::load_from_str(template)?;
    let doc = docs.pop().unwrap();

    let yaml = doc.as_hash().unwrap();
    let mut result = Hash::new();

    for (section, value) in yaml {
        if section == &Yaml::String("Resources".into()) {
            let resources = value.as_hash().unwrap();
            let mut new_resources = Hash::new();
            for (logical_id, resource) in resources {
                let schema = schema_for(resource)?;
                let props = properties(resource).unwrap();
                let clean = remove_mutually_exclusive(props, &schema.as_hash().unwrap());

                new_resources.insert(logical_id.clone(), Yaml::Hash(clean));
            }
            result.insert(Yaml::String("Resources".into()), Yaml::Hash(new_resources));
        } else {
            result.insert(section.clone(), value.clone());
        }
    }

    Ok(Yaml::Hash(result))
}

fn schema_for(resource: &Yaml) -> anyhow::Result<Yaml> {
    let resource_type = resource_type(resource);
    let filename = type_to_filename(resource_type.unwrap());
    let file = SCHEMAS_DIR.get_file(filename.as_str()).unwrap();
    let schema = file.contents_utf8().unwrap();
    let mut docs = YamlLoader::load_from_str(schema)?;
    Ok(docs.pop().unwrap())
}

fn properties(resource: &Yaml) -> Option<&Hash> {
    resource
        .as_hash()?
        .get(&Yaml::String("Properties".into()))?
        .as_hash()
}

fn resource_type(resource: &Yaml) -> Option<&str> {
    resource
        .as_hash()?
        .get(&Yaml::String("Type".into()))?
        .as_str()
}

fn remove_mutually_exclusive(data: &Hash, schema: &Hash) -> Hash {
    // TODO Recursive processing. For now, we are only sanitizing the top level.
    // TODO `data` should be &Yaml and then we pattern match against the possible types.
    //  Primitives are just cloned, and for hashes and maps, we make a recursive call

    let all_properties = schema
        .get(&Yaml::String("properties".into()))
        .and_then(|props| props.as_hash())
        .map(|props| props.keys().collect_vec())
        .unwrap();

    // TODO Move this chunk of code, up to the computation of remaining properties, to a separate function
    // Compute a set of property sets. All elements of a given property set are mutually exclusive
    // to all other elements of the same set.
    let mut property_sets = UnionFind::from_iter(all_properties.iter().copied());
    if let Some(dependent_excluded) = schema.get(&Yaml::String("dependentExcluded".into())) {
        for (prop, other_props) in dependent_excluded.as_hash().unwrap() {
            for other_prop in other_props.as_vec().unwrap() {
                property_sets.union(prop, other_prop).unwrap();
            }
        }
    }

    // Making sure that the final set of properties is a subset of the original properties
    let mut temp: HashMap<usize, &Yaml> = HashMap::new();
    for (name, value) in data {
        let x = property_sets.find_index(&name).unwrap();
        temp.insert(x, name);
    }
    let remaining_props = temp.values().cloned().collect_vec();

    let mut result = Hash::new();
    for prop in remaining_props {
        let value = data.get(prop).unwrap();
        result.insert(prop.clone(), value.clone());
    }

    result
}

fn type_to_filename(typ: &str) -> String {
    format!(
        "{}.json",
        typ.split("::")
            .map(|part| part.to_lowercase())
            .collect_vec()
            .join("_")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let template = r#"
Parameters:
  DbSubnetIpBlocks:
    Description: "Comma-delimited list of three CIDR blocks"
    Type: CommaDelimitedList
    Default: "10.0.48.0/24, 10.0.112.0/24, 10.0.176.0/24"
Resources:
   myInstance:
     Type: 'AWS::EC2::Instance'
     Properties:
        ImageId: ami-0a70b9d193ae8a799
        SubnetId:
            Ref: "PublicSubnet"
        NetworkInterfaces:
          - AssociatePublicIpAddress: "true"
            DeviceIndex: "0"
            GroupSet:
              - Ref: "myVPCEC2SecurityGroup"
            SubnetId:
              Ref: "PublicSubnet"
"#;
        let result = clean_template(&template.to_string()).unwrap();

        // TODO Assertions
        dbg!(&result);
    }
}
