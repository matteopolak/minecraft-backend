diesel::table! {
	account (id) {
		id -> Int4,
		username -> Text,
		password -> Text,
	}
}

diesel::table! {
	like (username, user_id) {
		username -> Text,
		user_id -> Int4,
	}
}

diesel::table! {
	name (username) {
		username -> Text,
		popularity -> Float8,
		definition -> Array<Text>,
		frequency -> Float8,
		length -> Int4,
		updating -> Bool,
		tags -> Array<Text>,
		status -> Int2,
		verified_at -> Timestamptz,
		checked_at -> Timestamptz,
		updated_at -> Timestamptz,
		created_at -> Timestamptz,
	}
}

diesel::table! {
	proxy (id) {
		id -> Int4,
		address -> Text,
		port -> Int4,
		username -> Nullable<Text>,
		password -> Nullable<Text>,
		note -> Nullable<Text>,
	}
}

diesel::table! {
	snipe (username) {
		username -> Text,
		needed -> Int2,
		count -> Int2,
		email -> Text,
		password -> Text,
		created_at -> Timestamptz,
	}
}

diesel::table! {
	user (id) {
		id -> Int4,
		key -> Text,
	}
}

diesel::allow_tables_to_appear_in_same_query!(account, like, name, proxy, snipe, user,);
