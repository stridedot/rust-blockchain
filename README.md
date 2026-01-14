## 区块链交易与存储时序图
1. 准备阶段：你生成“锁”
- 你：在钱包里生成一对密钥（私钥/公钥）。
- 你：通过公钥哈希生成你的接收地址。
- 你 $\rightarrow$ 发送方：你把地址字符串（比如通过微信或交易所界面）发送给对方。

2. 构建阶段：发送方写“支票”发送方：准备好要支付的 value。
- 发送方：调用你的代码 TXOutput::new(value, address)。
- 发送方：程序内部执行 lock(address)，将地址拆解为 pub_key_hash。
- 发送方：使用他自己的私钥对这笔交易进行签名（证明这笔钱是他合法支出的）。
- 发送方 $\rightarrow$ 节点：将完整的 Transaction 广播到全网。

3. 共识阶段：矿工打包（PoW）
- 节点：接收交易并验证签名是否合法。
- 节点：将这笔交易和其他交易一起塞进一个 Block。
- 节点：启动 ProofOfWork。不断尝试不同的 nonce，直到哈希值满足难度要求。
- 节点：一旦挖矿成功，该区块正式合法化。

4. 持久化阶段：存入磁盘
- 节点：调用你写的 Blockchain::add_block(block)。
- 节点 $\rightarrow$ sled：调用 update_blocks_tree。
    - 事务开始：将 Block（含 TXOutput）存入数据库。
    - 更新索引：将 TIP_BLOCK_HASH_KEY 指向这个新块。
    - 事务提交：数据永久落盘。

5. 确认阶段：你看到余额
- 你：刷新钱包，钱包扫描区块链数据库，发现有一个 TXOutput 的 pub_key_hash 正好匹配你的公钥哈希。
- 你：余额增加，交易成功！

## 区块链中的 hash
### 类别
1. Block 中的 hash (身份牌)
- 本质：这个区块所有数据的“数字指纹”。它是由区块头（版本、父哈希、默克尔根、时间戳、难度、Nonce）计算出来的。
- 唯一性：只要区块里的任何一个交易变了一个比特，这个 hash 就会天差地别。
- 作用：它是区块的唯一标识符。

2. BlockChain 中的 tip_hash (指向标)
- 本质：一个指针或书签。它始终存储着“当前最长链上最后一个区块”的哈希值。
- 动态性：随着新块的加入，tip_hash 会不断更新。
- 作用：它告诉程序：“如果我们要继续挖矿，应该接在哪个块后面”以及“如果我们要查询余额，应该从哪个块开始倒着找”。

3. sled 数据库中的 hash (存储索引/Key)
- 本质：数据库的 Key。
- 关系：在 sled 中，我们通常以 Block 的哈希值作为 Key，以 Block 序列化后的字节流作为 Value。
- 作用：让我们能以 $O(1)$ 的速度通过一个哈希值从硬盘里取出整个区块对象。

### 关系
这种关系的生命周期是这样的：
1. 产生：Block 被挖出来，计算出自己的 Block.hash。
2. 存储：
- 调用 db.insert(Block.hash, Block_bytes)。此时，Block.hash 成为了数据库的 Key。
- 这就是你说的“sled 存储的 hash”，它和 Block 内部的 hash 是同一个值。
3. 更新：
- 由于这个块是最新的，系统调用 set_tip_hash(Block.hash)。
- 此时，Blockchain.tip_hash 指向了这个新块。
- 同时，数据库中一个特殊的 Key（TIP_BLOCK_HASH_KEY）对应的值也被更新为这个 Block.hash。

### 对比
概念	             存储位置	                稳定性	    比喻
Block.hash	         结构体字段 / 数据库 Key	永久固定	身份证号
Blockchain.tip_hash	 内存 (RwLock)	           随新块变动	班级里最后一名进教室的同学
sled 中的 hash	     磁盘索引	                永久固定	档案柜上的标签
