// 引入 RLTK 库中的随机数生成器
use rltk::RandomNumberGenerator;

// 表示一个随机项，包括名称和权重
pub struct RandomEntry {
    name : String,   // 项目名称
    weight : i32     // 权重值，决定被选中的概率
}

// 实现 RandomEntry 的构造方法
impl RandomEntry {
    // 创建一个新的随机项
    pub fn new<S:ToString>(name: S, weight: i32) -> RandomEntry {
        RandomEntry{ name: name.to_string(), weight }
    }
}

// 定义一个随机表，用于存储多个随机项及其总权重
#[derive(Default)]
pub struct RandomTable {
    entries : Vec<RandomEntry>, // 所有的随机项
    total_weight : i32          // 所有项的权重总和
}

// 实现 RandomTable 的方法
impl RandomTable {
    // 创建一个新的空随机表
    pub fn new() -> RandomTable {
        RandomTable{ entries: Vec::new(), total_weight: 0 }
    }

    // 向随机表中添加一个新项
    pub fn add<S:ToString>(mut self, name : S, weight: i32) -> RandomTable {
        if weight > 0 {
            // 累加总权重
            self.total_weight += weight;
            // 添加新项到 entries 列表
            self.entries.push(RandomEntry::new(name.to_string(), weight));
        }
        self // 返回自身以支持链式调用
    }

    // 从随机表中根据权重随机选择一个项
    pub fn roll(&self, rng : &mut RandomNumberGenerator) -> String {
        // 如果没有任何项，返回 "None"
        if self.total_weight == 0 { return "None".to_string(); }

        // 在权重总和范围内掷骰子，得到一个随机数（从 0 开始）
        let mut roll = rng.roll_dice(1, self.total_weight) - 1;
        let mut index : usize = 0;

        // 遍历 entries，直到找到对应的项
        while roll > 0 {
            // 如果当前项的权重大于等于 roll，则选中该项
            if roll < self.entries[index].weight {
                return self.entries[index].name.clone();
            }

            // 否则减去当前项的权重，继续查找下一项
            roll -= self.entries[index].weight;
            index += 1;
        }

        // 如果没有找到，返回 "None"
        "None".to_string()
    }
}
