begin;

drop schema if exists substream1 cascade;

create schema if not exists substream1;

-- bytea to numeric:
-- from: https://stackoverflow.com/questions/3103242/inserting-text-string-with-hex-into-postgresql-as-a-bytea
CREATE OR REPLACE FUNCTION public.bytea2numeric(_b BYTEA) RETURNS NUMERIC AS $$
DECLARE
    _n NUMERIC := 0;
BEGIN
    FOR _i IN 0 .. LENGTH(_b)-1 LOOP
        _n := _n*256+GET_BYTE(_b,_i);
    END LOOP;
    RETURN _n;
END;
$$ LANGUAGE PLPGSQL IMMUTABLE STRICT;


-- Accounts table:
create table if not exists substream1.accounts
(
    address      text not null check (length(address) = 40) constraint accounts_pk primary key
);

comment on table substream1.accounts is 'account addresses to index transactions for';


-- Convert hex values to postgres numeric:
create or replace function substream1.fn_set_value_numeric() returns trigger as $$
begin
    new.value_num = public.bytea2numeric(decode(new.value, 'hex'));
    return new;
end;
$$ language plpgsql;


create table if not exists substream1.transfer_counts
(
    address       text not null default '' check(address = '' or length(address) = 40),
    token_address text not null check (length(token_address) = 40 or length(token_address) = 3),
    token_id      text not null default '',

    tx_count      bigint not null default 0 check(tx_count >= 0),

    primary key (token_address, token_id, address)
);

comment on table substream1.transfer_counts is 'Keeps track of the number of transactions for each token, for each account';

create index transfer_counts_address_idx on substream1.transfer_counts using btree (address);


-- Transfers table:
create table if not exists substream1.token_transfers
(
    log_index    integer not null check (log_index >= 0),
    call_index   integer not null check (call_index >= 0),
    "timestamp"  integer not null check("timestamp" > 1436940000),
    block_number integer not null check (block_number >= 0),
    value_num    numeric not null check (value_num >= 0),
    token_address text not null check (length(token_address) = 40),
    token_id     text not null default '',
    tx_hash      text not null check (length(tx_hash) = 64),
    from_address text not null check (length(from_address) = 40),
    to_address   text not null check (length(to_address) = 40),
    value        text not null,

    primary key (tx_hash, log_index)
);

create index token_transfers_to_addr_idx on substream1.token_transfers(to_address);
create index token_transfers_from_addr_idx on substream1.token_transfers(from_address);
create index token_transfers_block_num_ordinal_idx on substream1.token_transfers(block_number, log_index);

comment on table substream1.token_transfers is 'Transfers of tokens: ERC-20, ERC-721, ERC-1155';
comment on column substream1.token_transfers.token_id is 'token ID for ERC-1155.  Set to empty string '''' for ERC-20 and ERC-721';
comment on column substream1.token_transfers.value is 'hex representation of token value transfered.  Converted to numeric in
value_num column on insert';

create index if not exists token_transfers_token_address_token_id_block_numbers on substream1.token_transfers (token_address, token_id, block_number); --, tx_index);

create or replace trigger tr_token_transfers_set_value_numeric
before insert on substream1.token_transfers for each row
    execute procedure substream1.fn_set_value_numeric();

-- Account balances table:
create table if not exists substream1.account_balances
(
    block_number  integer not null check (block_number > 0),
    ordinal       integer not null check (ordinal >= 0),
    address       text not null constraint accounts_fk references substream1.accounts(address) on delete restrict,
    token_address text check (length(token_address) = 3 OR length(token_address) = 40),
    token_id      text not null default '',
    balance       numeric not null default 0, --  check (balance >= 0), -- TODO??

    primary key(address, token_address, token_id, block_number, ordinal)
);

create index token_address_token_id_block_num on substream1.account_balances (token_address, token_id, block_number, ordinal);

-- Running balance on update trigger
create or replace function substream1.fn_update_token_account_balances() returns trigger as $$
begin
    if exists(select from substream1.accounts where address = new.to_address) then
	insert into substream1.account_balances(address, token_address, token_id, block_number, ordinal, balance) values (
	    new.to_address,
	    new.token_address,
	    new.token_id,
	    new.block_number,
	    new.log_index,
	    coalesce(
		(
        	select balance from substream1.account_balances
		where address = new.to_address
		and token_address = new.token_address
		and token_id = new.token_id
		and ((block_number = new.block_number and ordinal < new.log_index) or block_number < new.block_number)
		order by block_number desc, ordinal desc
		limit 1
	        ), 0
	    ) + new.value_num
        ) on conflict(address, token_address, token_id, block_number, ordinal) do update set balance = excluded.balance;

	insert into substream1.transfer_counts (address, token_address, token_id, tx_count) values (
		new.to_address,
		new.token_address,
		new.token_id,
		coalesce(
			(select tx_count from substream1.transfer_counts
				where address = new.to_address
				and token_address = new.token_address
				and token_id = new.token_id
				limit 1
			), 0) + 1
	) on conflict(token_address, token_id, address) do update set tx_count = excluded.tx_count;
    end if;

    if exists(select from substream1.accounts where address = new.from_address) then
	insert into substream1.account_balances(address, token_address, token_id, block_number, ordinal, balance) values (
	    new.from_address,
	    new.token_address,
	    new.token_id,
	    new.block_number,
	    new.log_index,
	    coalesce(
		(
		select balance from substream1.account_balances
		where address = new.from_address
		and token_address = new.token_address
		and token_id = new.token_id
		and ((block_number = new.block_number and ordinal < new.log_index) or block_number < new.block_number)
		order by block_number desc, ordinal desc
		limit 1
		), 0
	    ) - new.value_num
	) on conflict(address, token_address, token_id, block_number, ordinal) do update set balance = excluded.balance;

	insert into substream1.transfer_counts (address, token_address, token_id, tx_count) values (
		new.from_address,
		new.token_address,
		new.token_id,
		coalesce(
			(select tx_count from substream1.transfer_counts
				where address = new.from_address
				and token_address = new.token_address
				and token_id = new.token_id
				limit 1
			), 0) + 1
	) on conflict(token_address, token_id, address) do update set tx_count = excluded.tx_count;
    end if;

    -- update transfer stats for all addresses
    if exists(select from substream1.accounts where address = new.from_address) or exists(select from substream1.accounts where address = new.to_address) then
	insert into substream1.transfer_counts (token_address, token_id, tx_count) values (
		new.token_address,
		new.token_id,
		coalesce(
			(select tx_count from substream1.transfer_counts
				where address = ''
				and token_address = new.token_address
				and token_id = new.token_id
				limit 1
			), 0) + 1
	) on conflict(token_address, token_id, address) do update set tx_count = excluded.tx_count;
    end if;

    return new;
end;
$$ language plpgsql;

create or replace trigger tr_update_token_account_balances
after insert on substream1.token_transfers for each row
    execute procedure substream1.fn_update_token_account_balances();


-- Balance functions:
create or replace function substream1.account_balance(text, text, text, bigint)
returns numeric
language plpgsql
as
$$
declare
   bal numeric;
begin
  select balance into bal from substream1.account_balances ab
      where ab.address = $1
      and ab.token_address = $2
      and ab.token_id = $3
      and ab.block_number <= $4
      order by block_number desc, ordinal desc limit 1;
  return coalesce(bal,0);
end;
$$;

comment on function substream1.account_balance is 'Returns account balance at block height.
account_balance(address, token_address, token_id, block_number)';

create or replace function substream1.account_balance(text, text, text, bigint, int)
returns numeric
language plpgsql
as
$$
declare
   bal numeric;
begin
  select balance into bal from substream1.account_balances ab
      where ab.address = $1
      and ab.token_address = $2
      and ab.token_id = $3
      and (
	ab.block_number <= $4
	or (
	  ab.block_number = $4 and ab.ordinal <= $5
        )
      )
      order by block_number desc, ordinal desc limit 1;
  return coalesce(bal,0);
end;
$$;

--comment on function substream1.account_balance is 'Returns account balance at block height.
--account_balance(address, token_address, token_id, block_number)';

create or replace function substream1.all_accounts_balance(text, text, bigint)
returns numeric
language plpgsql
as
$$
declare
   bal numeric;
begin
  select  sum(bal.account_balance) into bal from substream1.accounts a,
  lateral (select account_balance from substream1.account_balance(a.address, $1, $2, $3)) bal;
  return coalesce(bal,0);
end;
$$;

comment on function substream1.all_accounts_balance(text,text,bigint) is 'Returns sum of all accounts balance at block height.
all_accounts_balance(address, token_address, token_id, block_number)';

create or replace function substream1.all_accounts_balance(text, text, bigint, int)
returns numeric
language plpgsql
as
$$
declare
   bal numeric;
begin
  select  sum(bal.account_balance) into bal from substream1.accounts a,
  lateral (select account_balance from substream1.account_balance(a.address, $1, $2, $3, $4)) bal;
  return coalesce(bal,0);
end;
$$;

comment on function substream1.all_accounts_balance(text,text,bigint,int) is 'Returns sum of all accounts balance at block height.
all_accounts_balance(address, token_address, token_id, block_number)';

-- Value transfers:
create table if not exists substream1.value_transfers
(
    reason       integer not null check (reason >= 0 and reason <= 16),
    call_index   integer not null check (call_index >= 0),
    tx_index     integer not null check (tx_index >= 0),
    "timestamp"  integer not null check("timestamp" > 1436940000),
    block_number integer not null check (block_number > 0),
    value        text not null,
    value_num    numeric not null check(value_num >= 0),
    hash      text not null check(length(hash) = 64),
    from_address text not null check (length(from_address) = 0 or length(from_address) = 40),
    to_address   text not null check (length(to_address) = 0 or length(to_address) = 40),

    check ( not (length(to_address) = 0 and length(from_address) = 0)),
    primary key(hash, call_index)
);

create index value_transfers_to_addr_idx on substream1.value_transfers(to_address);
create index value_transfers_from_addr_idx on substream1.value_transfers(from_address);
create index value_transfers_block_num_ordinal_idx on substream1.value_transfers(block_number, tx_index);

comment on column substream1.value_transfers.hash is 'transaction or block hash, depending on reason';
comment on column substream1.value_transfers.reason is 'substreams_ethereum::pb::eth::v2::balance_change::Reason';

create index if not exists value_transfers_block_number on substream1.value_transfers(block_number);

create or replace trigger tr_value_transfers_set_value_numeric
before insert on substream1.value_transfers for each row
    execute procedure substream1.fn_set_value_numeric();


create or replace function substream1.fn_update_value_account_balances() returns trigger as $$
begin
    if exists(select from substream1.accounts where address = new.to_address) then
	insert into substream1.account_balances(address, token_address, block_number, ordinal, balance) values (
	    new.to_address,
	    'ETH',
	    new.block_number,
	    new.tx_index,
	    coalesce(
		(
        	select balance from substream1.account_balances
		where address = new.to_address
		and token_address = 'ETH'
		and token_id = ''
		and ((block_number = new.block_number and ordinal <= new.tx_index) or block_number < new.block_number)
		order by block_number desc, ordinal desc
		limit 1
	        ), 0
	    ) + new.value_num
        ) on conflict(address, token_address, token_id, block_number, ordinal) do update set balance = excluded.balance;

	insert into substream1.transfer_counts (address, token_address, token_id, tx_count) values (
		new.to_address,
		'ETH',
		'',
		coalesce(
			(select tx_count from substream1.transfer_counts
				where address = new.to_address
				and token_address = 'ETH'
				and token_id = ''
				limit 1
			), 0) + 1
	) on conflict(token_address, token_id, address) do update set tx_count = excluded.tx_count;
    end if;

    if exists(select from substream1.accounts where address = new.from_address) then
	insert into substream1.account_balances(address, token_address, block_number, ordinal, balance) values (
	    new.from_address,
	    'ETH',
	    new.block_number,
	    new.tx_index,
	    coalesce(
		(
		select balance from substream1.account_balances
		where address = new.from_address
		and token_address = 'ETH'
		and token_id = ''
		and ((block_number = new.block_number and ordinal <= new.tx_index) or block_number < new.block_number)
		order by block_number desc, ordinal desc
		limit 1
		), 0
	    ) - new.value_num
	) on conflict(address, token_address, token_id, block_number, ordinal) do update set balance = excluded.balance;

	insert into substream1.transfer_counts (address, token_address, token_id, tx_count) values (
		new.from_address,
		'ETH',
		'',
		coalesce(
			(select tx_count from substream1.transfer_counts
				where address = new.from_address
				and token_address = 'ETH'
				and token_id = ''
				limit 1
			), 0) + 1
	) on conflict(token_address, token_id, address) do update set tx_count = excluded.tx_count;
    end if;

    -- update transfer stats for all addresses
    if exists(select from substream1.accounts where address = new.from_address) or exists(select from substream1.accounts where address = new.to_address) then
	insert into substream1.transfer_counts (token_address, token_id, tx_count) values (
		'ETH',
		'',
		coalesce(
			(select tx_count from substream1.transfer_counts
				where address = ''
				and token_address = 'ETH'
				and token_id = ''
				limit 1
			), 0) + 1
	) on conflict(token_address, token_id, address) do update set tx_count = excluded.tx_count;
    end if;

    return new;
end;
$$ language plpgsql;

create or replace trigger tr_update_value_account_balances
after insert on substream1.value_transfers for each row
    execute procedure substream1.fn_update_value_account_balances();

-- Call Traces:
create table if not exists substream1.call_traces
(
    "index"    integer not null check ("index" >= 0),
    tx_hash    text not null check (length(tx_hash) = 64),
    trace      jsonb not null,

    -- foreign key to value_transfer?
    primary key(tx_hash, "index")
);

comment on table substream1.call_traces is 'Stores records of a JSON representation of the call stack
of every transaction which produced a value transfer or a token transfer';

-- Transfers View:
create or replace view substream1.transfers
as
(
    select
    log_index as ordinal,
    "timestamp",
    block_number,
    value_num as "value",
    token_address,
    token_id,
    tx_hash as hash,
    from_address,
    to_address
    from substream1.token_transfers
)
union all
(
    select
    tx_index as ordinal,
    "timestamp",
    block_number,
    value_num as "value",
    'ETH' as token_address,
    '' as token_id,
    hash,
    from_address,
    to_address
    from substream1.value_transfers
);


-- Tokens Issued:
create table if not exists substream1.tokens_issued
(
    token_address text not null check (length(token_address) = 40),
    token_id      text not null default '',

    primary key(token_address, token_id)
);

create table if not exists substream1.tokens_issued_stats
(
    token_holders bigint  not null default 0 check(token_holders >= 0),
    tx_count      bigint  not null default 0 check(tx_count >= 0),

    minted        numeric not null default 0,
    burned        numeric not null default 0,

    block_number  integer not null check (block_number >= 0),

    token_address text not null check (length(token_address) = 40),
    token_id      text not null default '',

    total_supply  numeric not null default 0 check(total_supply >= 0),

    primary key (token_address, token_id, block_number)
);

create table if not exists substream1.tokens_issued_transfers (
    log_index    int not null check (log_index >= 0),
    call_index   int not null check (call_index >= 0),
    "timestamp"  integer not null check("timestamp" > 1436940000),
    block_number integer not null check (block_number >= 0),
    value_num    numeric not null check (value_num >= 0),
    token_address text check (length(token_address) = 0 or length(token_address) = 40),
    token_id      text check ((length(token_address) = 0 and length(token_id) = 0) or length(token_address) = 40),
    tx_hash      text not null check (length(tx_hash) = 64),
    from_address text not null check (length(from_address) = 40),
    to_address   text not null check (length(to_address) = 40),
    value        text not null,

    -- TODO: foreign key to tokens_issued
    primary key (tx_hash, log_index)
);

create index tokens_issued_transfers_block_num_log_idx on substream1.tokens_issued_transfers(token_address, token_id, block_number, log_index);


create or replace trigger tr_issued_token_transfers_set_value_numeric
before insert on substream1.tokens_issued_transfers for each row
    execute procedure substream1.fn_set_value_numeric();

-- Running balance on insert trigger
create or replace function substream1.fn_update_issued_tokens_transfered() returns trigger as $$
declare
    prev_to_bal   numeric;
    prev_from_bal numeric;
    prev_token_holders_count bigint;
    token_holder_count_changed boolean;
begin
   prev_to_bal = coalesce(
	    (select balance from substream1.tokens_issued_holder_balances
	    where address = new.to_address
	    and token_address = new.token_address
	    and token_id = new.token_id
	    order by block_number desc
	    limit 1
	    ), 0);

    prev_from_bal = coalesce(
	    (select balance from substream1.tokens_issued_holder_balances
	    where address = new.from_address
	    and token_address = new.token_address
	    and token_id = new.token_id
	    order by block_number desc
	    limit 1
	    ), 0);

    prev_token_holders_count = coalesce(
	(select token_holders from substream1.tokens_issued_stats
	where token_address = new.token_address
	and token_id = new.token_id
	order by block_number desc
	limit 1
        ), 0);

    token_holder_count_changed = false;
    if (prev_to_bal = 0 and new.value_num > 0) then
	prev_token_holders_count = prev_token_holders_count + 1;
    	token_holder_count_changed = true;
    end if;

    if ((prev_from_bal - new.value_num) = 0) then
	prev_token_holders_count = prev_token_holders_count - 1;
    	token_holder_count_changed = true;
    end if;

    -- update balance history:
    insert into substream1.tokens_issued_holder_balances(block_number, address, token_address, token_id, balance) values (
	new.block_number,
	new.to_address,
	new.token_address,
	new.token_id,
	prev_to_bal + new.value_num
    ) on conflict(address, token_address, token_id, block_number) do update set balance = excluded.balance;
    insert into substream1.tokens_issued_holder_balances(block_number, address, token_address, token_id, balance) values (
	new.block_number,
	new.from_address,
	new.token_address,
	new.token_id,
	prev_from_bal - new.value_num
    ) on conflict(address, token_address, token_id, block_number) do update set balance = excluded.balance;

    -- update holders list
    insert into substream1.tokens_issued_holders(address, token_address, token_id, balance) values (
	new.to_address,
	new.token_address,
	new.token_id,
	coalesce(
		(select balance from substream1.tokens_issued_holders
			where address = new.to_address
			and token_address = new.token_address
			and token_id = new.token_id
			limit 1
		), 0) + new.value_num
    ) on conflict(address, token_address, token_id) do update set balance = excluded.balance;

    insert into substream1.tokens_issued_holders(address, token_address, token_id, balance) values (
	new.from_address,
	new.token_address,
	new.token_id,
	coalesce(
		(select balance from substream1.tokens_issued_holders
			where address = new.from_address
			and token_address = new.token_address
			and token_id = new.token_id
			limit 1
		), 0) - new.value_num
    ) on conflict(address, token_address, token_id) do update set balance = excluded.balance;

    -- check for burns/mints
    -- mint:
    if ((select substream1.is_burn_address(new.from_address)) and not (select substream1.is_burn_address(new.to_address))) then
        insert into substream1.tokens_issued_stats (token_address, token_id, block_number, total_supply, token_holders, tx_count, minted, burned) values (
            new.token_address,
            new.token_id,
	    new.block_number,
            coalesce((select total_supply from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1), 0) + new.value_num,
            prev_token_holders_count,
	    coalesce((select tx_count from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1), 0) + 1,
	    coalesce((select minted from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1), 0) + new.value_num,
	    coalesce((select burned from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1), 0)
        ) on conflict(token_address, token_id, block_number) do update set total_supply = excluded.total_supply, token_holders = excluded.token_holders, tx_count = excluded.tx_count, minted = excluded.minted;

    -- burn:
    elsif ((select substream1.is_burn_address(new.to_address)) and not (select substream1.is_burn_address(new.from_address))) then
        insert into substream1.tokens_issued_stats (token_address, token_id, block_number, total_supply, token_holders, tx_count, minted, burned) values (
            new.token_address,
            new.token_id,
	    new.block_number,
            coalesce((select total_supply from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1), 0) - new.value_num,
            prev_token_holders_count,
	    coalesce((select tx_count from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1), 0) + 1,
	    coalesce((select minted from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1), 0),
	    coalesce((select burned from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1), 0) - new.value_num
        ) on conflict(token_address, token_id, block_number) do update set total_supply = excluded.total_supply, token_holders = excluded.token_holders, tx_count = excluded.tx_count, burned = excluded.burned;
    elsif token_holder_count_changed then
	-- regular transfer:
        insert into substream1.tokens_issued_stats (token_address, token_id, block_number, total_supply, token_holders, tx_count, minted, burned) values (
            new.token_address,
            new.token_id,
	    new.block_number,
            (select total_supply from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1),
            prev_token_holders_count,
	    coalesce((select tx_count from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1), 0) + 1,
	    coalesce((select minted from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1), 0),
	    coalesce((select burned from substream1.tokens_issued_stats where token_address = new.token_address and token_id = new.token_id order by block_number desc limit 1), 0)
        ) on conflict(token_address, token_id, block_number) do update set total_supply = excluded.total_supply, token_holders = excluded.token_holders, tx_count = excluded.tx_count;
    end if;

    return new;
end;
$$ language plpgsql;

create or replace trigger tr_update_issued_tokens_transfered
after insert on substream1.tokens_issued_transfers for each row
    execute procedure substream1.fn_update_issued_tokens_transfered();


-- Issued Token holders:
create table if not exists substream1.tokens_issued_holder_balances
(
    block_number  integer not null check (block_number > 0),
    address       text not null check (length(address) = 40),
    token_address text check (length(token_address) = 3 OR length(token_address) = 40),
    token_id      text not null default '',
    balance       numeric not null default 0,

    primary key(address, token_address, token_id, block_number)
);

comment on table substream1.tokens_issued_holder_balances is 'Balance change history for tokens issued';

create table if not exists substream1.tokens_issued_holders
(
    address       text not null check (length(address) = 40),
    token_address text check (length(token_address) = 3 OR length(token_address) = 40),
    token_id      text not null default '',
    balance       numeric not null default 0,

    primary key(address, token_address, token_id)
);

comment on table substream1.tokens_issued_holders is 'Issued token holders and current balance';

create index tokens_issued_holders_balances on substream1.tokens_issued_holders(token_address, token_id, balance);


-- Utlity functions:
create or replace function substream1.is_burn_address(text)
returns boolean
as $$
begin
    if (
	   $1 = '0000000000000000000000000000000000000000'
	or $1 = '000000000000000000000000000000000000dead'
    )
        then return true;
    else
        return false;
    end if;
end;
$$
language plpgsql;

-- substreams-sink-postgres cursors:
create table if not exists substream1.cursors
(
    id         text not null constraint cursor_pk primary key,
    cursor     text,
    block_num  bigint,
    block_id   text
);

commit;
