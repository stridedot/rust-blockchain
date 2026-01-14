## TXInput
### 字段含义
1、txid: Vec<u8>
- 含义： 被引用的「上一笔交易」的哈希（Transaction ID）
- 作用：指明我要花费的是哪一笔交易

在 UTXO 表中的定位方式：
(txid, vout) → 唯一确定一个 UTXO

2️、vout: usize
- 含义：被引用交易中第几个输出（output index）
- 作用：一笔交易可以有多个输出，vout 用来指定该交易的第几个输出

示例：
```text
Tx A:
  output[0] → 给 Alice
  output[1] → 找零给 Bob

Tx B 的 input:
  txid = hash(A)
  vout = 1   // 花费 A 的第 1 个输出
```

3、signature: Vec<u8>

- 含义：对当前交易内容的数字签名
- 作用（核心安全点）：证明：发起交易的人，确实拥有被花费 UTXO 的私钥

防止他人伪造交易

验证逻辑（简化）：
```text
verify(
    signature,
    transaction_data,
    pub_key
) == true
```

注意：

签名的是“交易本身”（通常是去掉 signature 的交易副本）

每个 input 都有自己的 signature（Bitcoin 即如此）

4️、pub_key: Vec<u8>
- 含义：花费该 UTXO 所使用的公钥
- 作用：用来验证 signature，与 Output 中的“锁定条件”进行匹配

在你的代码中：
```rust
pub fn use_key(&self, pub_key: &[u8]) -> bool {
    let locking_hash = wallet::hash_pub_key(self.pub_key.as_slice());
    locking_hash == pub_key.to_vec()
}
```

这反映的是典型的 Pay-to-PubKey-Hash (P2PKH) 模型：

Output 里存的是：hash(pub_key)
Input 里提供的是：pub_key + signature

验证时：

hash(input.pub_key) 是否等于 output 中的 pub_key_hash

再用 pub_key 验证 signature

### 整体含义

一个 TXInput 可以理解为一句完整的话：

“我使用公钥 pub_key，对这笔交易做了签名 signature，来花费交易 txid 中第 vout 个输出。”

与你当前代码设计的对应关系
| 字段 |	UTXO 模型角色 |	是否必须 |
| ---------- | ---------- | --------- |
| txid |	定位旧交易 |	是 |
| vout |	定位具体输出 |	是 |
| signature |	解锁脚本 / 所有权证明 |	是（coinbase 除外） |
| pub_key |	身份声明 |	是 |

### 总结一句话
TXInput 的四个字段共同完成了 **UTXO 定位 + 所有权证明 + 解锁条件满足** 这三件区块链交易中最核心的事情。

### 详解 use_key
一、一句话版：
- pub_key 是“我是谁”，
- signature 是“我确实同意并授权这次花钱”，
- use_key() 只负责验证 “我是不是这笔钱的合法主人”，它不负责验证签名本身。

二、把“花 UTXO”类比成现实世界的事
假设有一张 银行支票：

**Output（钱被锁住的时候）**
银行记录的是：

> “这笔钱只能被「某个身份证号的 hash」对应的人取走”

⚠️ 注意：
银行不会存你的身份证原件，只存 hash。

三、现在你要花这笔钱（TXInput 出现了）

你必须带三样东西去银行：

1️⃣ 你说：这钱是从哪来的？
```text
txid + vout
```

> “我要取的是第 X 号支票”

2️⃣ 你说：我是谁？
```text
pub_key
```

这相当于：

> “这是我的身份证原件”

3️⃣ 你说：我不是偷的，我本人同意这次取钱
signature


这相当于：

> “这是我当场签的字”


四、回到代码：use_key() 到底在干嘛？
现在这段代码：
```rust
pub fn use_key(&self, pub_key: &[u8]) -> bool {
    let locking_hash = wallet::hash_pub_key(self.pub_key.as_slice());
    locking_hash == pub_key.to_vec()
}
```

第 1 行
```rust
let locking_hash = wallet::hash_pub_key(self.pub_key.as_slice());
```

意思是：

> “把 Input 里带来的公钥，算一次 hash”

也就是：

> hash(身份证原件)

第 2 行
```rust
locking_hash == pub_key.to_vec()
```


这里的 pub_key 不是 Input 里的公钥，而是：

> Output 里存的 pub_key_hash

也就是：

> hash(身份证原件) == 银行里存的 hash

✅ 如果相等，说明什么？

说明：

> 这个人，确实是当初锁定这笔钱的人

⚠️ 到这里为止，只验证了 “你是谁”，
还没验证你有没有授权这次交易。


五、那 signature 到底什么时候用？

这是你现在缺失但必然要有的一步。

真正完整的花钱验证流程是：

第一步：身份匹配（你现在的 use_key()）
hash(input.pub_key) == output.pub_key_hash


✔️ 你是合法身份

第二步：签名验证（你现在还没写）
verify_signature(
    input.signature,
    tx_data,
    input.pub_key
)


✔️ 你确实用私钥签过这笔交易


## TXInput+TXOutput
### 流程图
```scss
  [Wallet / 用户]
  ┌───────────────────────┐
  │  公钥 (pub_key)       │
  │  私钥 (private_key)   │
  └───────────────────────┘
            │
            │ hash_pub_key(pub_key)
            ▼
  ┌───────────────────────┐
  │  地址 (Address)       │
  │ Base58(version +      │
  │ pub_key_hash + checksum)
  └───────────────────────┘
            │
            │ lock(address)
            ▼
  ┌───────────────────────┐
  │ TXOutput               │
  │ ┌───────────────────┐ │
  │ │ value             │ │  ← 金额
  │ │ pub_key_hash      │ │  ← 锁定条件
  │ └───────────────────┘ │
  └───────────────────────┘
            ▲
            │  查找 UTXO: output.is_locked_with_key(pub_key_hash)
            │
            │ hash(input.pub_key) == output.pub_key_hash
            │
  ┌───────────────────────┐
  │ TXInput                │
  │ ┌───────────────────┐ │
  │ │ txid              │ │  ← 引用的交易
  │ │ vout              │ │  ← 引用的输出索引
  │ │ pub_key           │ │  ← 用于证明身份
  │ │ signature         │ │  ← 对交易签名
  │ └───────────────────┘ │
  └───────────────────────┘
            │
            │ 验证签名:
            │ verify(signature, tx_data, pub_key)
            ▼
  ┌───────────────────────┐
  │ 验证通过 = 可花费 Output │
  └───────────────────────┘
```

### 流程说明

1、钱包生成公私钥对
- pub_key 用于收款，private_key 用于签名

2、 地址生成
- 对 pub_key 做哈希得到 pub_key_hash
- 生成 Base58 地址用于展示

3、创建 TXOutput
- 钱包指定收款地址
- Output 存金额和 pub_key_hash（锁定条件）

4、创建 TXInput 花钱
- 引用某笔 Output (txid + vout)
- 提供公钥 (pub_key) 和签名 (signature)

5、验证逻辑
- `hash(pub_key) == output.pub_key_hash` → 验证身份
- `verify(signature, tx_data, pub_key)` → 验证授权

6、通过验证后
- 这笔 Output 可以被花掉
- 新交易产生新的 Output（新的 pub_key_hash）

这张图清楚展示了：
- 地址 → pub_key_hash → Output → Input → 验证
- use_key() / is_locked_with_key() 只检查身份
- signature 验证才保证授权


## UTXO 模型
```rust
pub struct TXInput {
    txid: Vec<u8>,
    vout: usize,
    signature: Vec<u8>,
    pub_key: Vec<u8>,
}

pub struct TXOutput {
    value: i32,
    pub_key_hash: Vec<u8>,
}

pub struct Transaction {
    id: Vec<u8>,
    vin: Vec<TXInput>,
    vout: Vec<TXOutput>,
}
```

### 一、先给一句“总定义”

Transaction 做两件事：
- 用 vin 指向并消费历史上的 TXOutput
- 用 vout 创建新的 TXOutput，供未来交易消费

所有字段的意义，都是围绕这两件事展开的。

### 二、TXOutput：钱是“怎么存在的”
```rust
pub struct TXOutput {
    value: i32,
    pub_key_hash: Vec<u8>,
}
```

**语义解释**
一个 TXOutput 表示：
> “某一笔钱 + 它未来可以被谁花”

| 字段	| 含义|
| -- | -- |
| value | 金额 |
| pub_key_hash | 能解锁这笔钱的“身份” |

重要结论（一定要记住）
> TXOutput 自身是“没有身份的”
> 它必须通过 (txid, vout_index) 才能被唯一定位

示例：一笔交易的输出
```text
txA:
  vout[0]: 5 BTC → Alice
  vout[1]: 3 BTC → Bob
```

在代码里：
```
txA.vout[0] = TXOutput { value: 5, pub_key_hash: hash(Alice) };
txA.vout[1] = TXOutput { value: 3, pub_key_hash: hash(Bob) };
```

### 三、TXInput：钱怎么被“花掉”
```rust
pub struct TXInput {
    txid: Vec<u8>,
    vout: usize,
    signature: Vec<u8>,
    pub_key: Vec<u8>,
}
```

**语义解释**
一个 TXInput 表示：
> “我要花某一笔已经存在的钱”

| 字段	| 含义|
| -- | -- |
| txid | 指向哪一笔历史交易 |
| vout | 指向该交易的第几个输出 |
| signature | 对当前交易的签名 |
| pub_key | 用来验证 signature 的公钥 |

**核心定位规则（非常重要）**
TXInput 精确引用的是：
```text
(txid, vout)
```

也就是：
> “txid 这笔交易产生的第 vout 个 TXOutput”

**示例：花掉 Alice 的那 5 BTC**
```text
txB:
  vin[0] -> txA, vout = 0
```

代码上是：
```rust
TXInput {
    txid: txA.id,
    vout: 0,
    signature: sign(txB),
    pub_key: AlicePubKey,
}
```

### 四、Transaction：状态迁移的最小单位
```rust
pub struct Transaction {
    id: Vec<u8>,
    vin: Vec<TXInput>,
    vout: Vec<TXOutput>,
}
```

**语义解释**

一个 Transaction 表示：
> “把若干笔旧钱（vin）
> 变成若干笔新钱（vout）”

### 五、一个完整、真实、合法的交易示例
**初始状态（UTXO）**
```text
txA:
  vout[0]: 5 BTC → Alice
  vout[1]: 3 BTC → Alice
```

**Alice 想支付 Bob 6 BTC**

她需要：
- 花掉 vout[0] = 5
- 花掉 vout[1] = 3
- 找零 2 BTC 给自己

** 构造交易 txB**
TXInput（引用历史输出）
```text
vin[0] -> txA:vout[0] (5 BTC)
vin[1] -> txA:vout[1] (3 BTC)
```

TXOutput（生成新输出）
```text
vout[0]: 6 BTC → Bob
vout[1]: 2 BTC → Alice
```

**用结构体表示**
```rust
Transaction {
    id: txB_id,
    vin: vec![
        TXInput { txid: txA_id, vout: 0, ... },
        TXInput { txid: txA_id, vout: 1, ... },
    ],
    vout: vec![
        TXOutput { value: 6, pub_key_hash: hash(Bob) },
        TXOutput { value: 2, pub_key_hash: hash(Alice) },
    ],
}
```

### 六、关键约束关系（你必须能在脑中画出来）
1️⃣ 引用关系
```text
TXInput  ----(txid, vout)--->  TXOutput
```

2️⃣ 唯一性规则（共识级）
同一个 (txid, vout)：
- 只能被引用一次
- 不论在同一交易中，还是不同交易中

3️⃣ 合法但常见的结构（重点）
```text
多个 TXInput
  ↓
可以引用
  ↓
同一个 txid 的不同 vout
```

这是你前面所有疑惑的根源。

### 七、把你现在的代码问题放回这个模型里

你当前 UTXO 设计是：
```text
UTXO:
  txid -> Vec<TXOutput>
```

而真实语义是：
```text
UTXO:
  (txid, vout) -> TXOutput
```

于是你被迫：
- 用 Vec index 模拟 vout
- 一边删一边改
- index 语义被破坏
- update 看起来“可以 insert”，但实际上不安全

### 八、一句话总结（你现在应该“通了”）

- TXOutput 是“钱的实体”
- TXInput 是“花钱的引用”
- Transaction 是“钱的状态迁移”
- UTXO 是“所有还没被花的钱”
