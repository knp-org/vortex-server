import sqlite3
import sys

def check_cast_data():
    try:
        conn = sqlite3.connect('vortex_server.db')
        cursor = conn.cursor()
        
        print("--- Database Schema for media ---")
        cursor.execute("PRAGMA table_info(media)")
        columns = cursor.fetchall()
        cast_col = None
        for col in columns:
            if col[1] == 'cast':
                cast_col = col
            print(col)
            
        if not cast_col:
            print("\nCRITICAL: 'cast' column DOES NOT EXIST in media table!")
            return

        print("\n--- Cast Data Sample ---")
        cursor.execute('SELECT id, title, "cast", provider_ids FROM media LIMIT 5')
        rows = cursor.fetchall()
        if not rows:
            print("No rows found.")
        else:
            for row in rows:
                cast_val = "NULL" if row[2] is None else "PRESENT"
                pid_val = row[3] if row[3] is not None else "NULL"
                print(f"ID: {row[0]}, Title: {row[1]}")
                print(f"  Cast: {cast_val}")
                print(f"  Provider IDs: {pid_val}")

        conn.close()
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    check_cast_data()
