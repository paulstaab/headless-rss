import argparse
from src.feed import update_all

def main():
    parser = argparse.ArgumentParser(description="Update all feeds in the database.")
    parser.add_argument("update", action="store_true", help="Update all feeds")
    args = parser.parse_args()

    if args.update:
        update_all()

if __name__ == "__main__":
    main()
