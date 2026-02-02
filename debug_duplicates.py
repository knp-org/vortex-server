import sqlite3

def check_duplicates():
    try:
        conn = sqlite3.connect('vortex_server.db')
        cursor = conn.cursor()
        
        print("--- Distinct Series Names ---")
        cursor.execute("SELECT series_name, COUNT(*) FROM media WHERE series_name IS NOT NULL GROUP BY series_name")
        rows = cursor.fetchall()
        for row in rows:
            print(f"'{row[0]}': {row[1]} episodes")
            
        print("\n--- Files with NULL series_name ---")
        cursor.execute("SELECT COUNT(*) FROM media WHERE series_name IS NULL AND media_type = 'series'")
        row = cursor.fetchone()
        print(f"Count: {row[0]}")

        conn.close()
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    check_duplicates()
