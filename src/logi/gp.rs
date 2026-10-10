use std::collections::BTreeMap;

use crate::{
    logi::mem::{Member, MemberInfo},
    phy::stg::Storage,
};

#[derive(Debug)]
pub struct Group {
    pub gp_info: GroupInfo,
    // relations
    pub mems: Vec<Member>,
}

#[derive(Debug)]
pub struct GroupInfo {
    pub gp_uuid: String,
}

pub fn build_group_map_from_storages(stgs: &[Storage]) -> BTreeMap<String, Group> {
    let mut gp_map: BTreeMap<String, Group> = BTreeMap::new();

    for stg in stgs {
        for cab in &stg.cabs {
            for mem_conf in &cab.cab_info.cab_conf.memberships {
                let gp_uuid = &mem_conf.group_uuid;
                // create if not exist
                let gp = gp_map.entry(gp_uuid.clone()).or_insert_with(|| Group {
                    gp_info: GroupInfo {
                        gp_uuid: gp_uuid.clone(),
                    },
                    mems: vec![],
                });

                gp.mems.push(Member {
                    mem_info: MemberInfo {
                        cab_info: cab.cab_info.clone(),
                        mem_conf: mem_conf.clone(),
                        stg_info: stg.stg_info.clone(),
                    },
                });
            }
        } 
    };

    gp_map
}
